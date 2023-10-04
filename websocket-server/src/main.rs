mod db;
mod handshake;

use anyhow::Result;
use db::{init_db, Db};
use handshake::{receive_http_request, send_websocket_upgrade_response, HttpRequest};
use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};

/*
오늘 무엇을 합니까?
저는 오늘 "Low-level WebSocket"을 할 것입니다.
뭔말이냐? 뭔말이긴 뭔말이야. 말 그대로입니다.
low level로 웹소켓을 구현하고, 그것으로 채팅을 구현하는 것.
= 라이브러리 안쓰겠다.
*/

#[tokio::main]
async fn main() -> Result<()> {
    let db = Arc::new(init_db().await?);

    let tcp_listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;

    let user_txs = std::sync::Arc::new(tokio::sync::Mutex::new(vec![]));
    // Arc 쓰는 이유: 언제 힙에서 제거해야하는지 알기 위해서!

    loop {
        let (tcp_stream, _) = tcp_listener.accept().await?;
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let id = generate_new_id();

        {
            let mut user_txs = user_txs.lock().await;
            user_txs.push(UserTx { id, tx });
        }
        // Q. 코파일럿이 굳이 이거를 Block{}을 만들어서 위 코드를 짠 이유는?

        start_user_loop(tcp_stream, rx, id, user_txs.clone(), db.clone());
    }

    // Q. 유저 5천명 들어오면, 스레드 몇개? 5천개
}

struct UserTx {
    id: u64,
    tx: tokio::sync::mpsc::Sender<String>,
}

type UserTxs = std::sync::Arc<tokio::sync::Mutex<Vec<UserTx>>>;

/*
    연결을 받으면
    스레드를 만들자!
    그럼 그 스레드 안에선 뭘 해요?
    연결된 Tcp Socket에 대한 WebSocket Handshake를 하구요,
    메시지를 받고요,
    다른 사람들에게 뿌려줘요.

    유저 A에 대한 쓰레드
    - 유저 A의 메시지를 받는중
       - Timeout을 걸어서, 받는데 기다리는 최대 시간을 정하자.
       // - Nonblocking 함수로 처리하자! << 이거는 잠시 머리속에서 빼놓자.
    유저 B에 대한 쓰레드
     - 이미 유저 B의 메시지 받았고, 그것을 다른 유저들에게 쏴줘야함.

    스레드는 언제까지 돌아야해? 언제 꺼져야해?
*/
fn start_user_loop(
    tcp_stream: TcpStream,
    rx: tokio::sync::mpsc::Receiver<String>,
    my_id: u64,
    user_txs: UserTxs,
    db: Arc<Db>,
) {
    tokio::spawn(async move {
        let _ = user_loop(tcp_stream, rx, my_id, user_txs.clone(), db).await;

        user_txs.lock().await.retain(|user_tx| user_tx.id != my_id);
    });
}

async fn user_loop(
    mut tcp_stream: TcpStream,
    mut rx: tokio::sync::mpsc::Receiver<String>,
    my_id: u64,
    user_txs: UserTxs,
    db: Arc<Db>,
) -> Result<()> {
    let request = receive_http_request(&mut tcp_stream).await?;

    if !request.is_websocket_upgrade_request() {
        handle_non_websocket_http_request(&mut tcp_stream, request, &db).await?;
        return Ok(());
    }

    send_websocket_upgrade_response(&mut tcp_stream, &request).await?;

    let mut partial_websocket_message = PartialWebsocketMessage {
        core_payload_length: None,
        extended_payload_length: None,
        masking_key: None,
        payload: None,
    };

    let (mut tcp_read, mut tcp_write) = tcp_stream.into_split();

    let close_notify = Arc::new(tokio::sync::Notify::new());
    let recv_task = tokio::spawn({
        let close_notify = close_notify.clone();
        async move {
            loop {
                match receive_user_message_and_send_to_other_users(
                    &mut tcp_read,
                    &mut partial_websocket_message,
                    my_id,
                    &user_txs,
                    &db,
                )
                .await
                {
                    Ok(_) => {
                        println!("user {my_id} send Message");
                        partial_websocket_message.clean_up();
                    }
                    Err(error) => match error {
                        ReceiveUserMessageError::Io(error) => {
                            todo!("에러처리 해야함 {:?}", error.kind())
                        }
                        ReceiveUserMessageError::NonSupported(error) => {
                            todo!("에러처리 해야함 {:?}", error)
                        }
                        ReceiveUserMessageError::Disconnected => {
                            close_notify.notify_one();
                            user_txs.lock().await.retain(|user_tx| user_tx.id != my_id);
                            break;
                        }
                        ReceiveUserMessageError::FailToSaveMessageToDb => {
                            println!("Fail to save message to db");
                            continue;
                        }
                    },
                }
            }
        }
    });

    let send_task = tokio::spawn(async move {
        match send_other_users_messages_to_user(&mut tcp_write, &mut rx, my_id, close_notify).await
        {
            Ok(_) => {}
            Err(error) => {
                todo!("에러처리 해야함 {:?}", error)
            }
        };
    });

    recv_task.await.unwrap();
    send_task.await.unwrap();

    Ok(())
}

async fn handle_non_websocket_http_request(
    tcp_stream: &mut TcpStream,
    request: HttpRequest,
    db: &Db,
) -> Result<()> {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => {
            let mut messages = db.list_messages(10).await?;
            messages.reverse();

            let message_lis = messages
                .into_iter()
                .map(|message| format!("<li>{}</li>", message))
                .collect::<Vec<_>>()
                .join("\n");

            /*
                생길 수 있는 버그
                1. DB에서 메시지를 긁어다가 사용자에게 보내줄 것.
                2. 근데 그 사이에 다른 유저가 메시지를 보냄.
                3. 하지만 이 사용자는 WebSocket을 연결하기 전인걸?
                4. 그러면 이 사용자는 html을 받고, WebSocket을 연결하기 전에 생긴 새로운 메시지들은 못받겠네?
            */

            let index_html = format!(
                "
            <html>
                <head>
                    <title>Chat</title>
                    <meta charset=\"utf-8\">
                </head>
                <body>
                    <input id=\"input\" type=\"text\"/>
                    <ul id=\"messages\">
                        {message_lis}
                    </ul>

                    <script>
                        const input = document.getElementById('input');
                        const messages = document.getElementById('messages');
                        const ws = new WebSocket(`ws://${{location.host}}/`);

                        // TODO: 내가 가지고 있는 가장 최근 메시지 이후로 또 온게 있으면 보내줘. 혹시 모르니까!

                        ws.addEventListener('message', (event) => {{
                            const message = event.data;
                            addMessageToList(message);
                        }});

                        input.addEventListener('keydown', (event) => {{
                            if (event.key === 'Enter') {{
                                const message = input.value;
                                input.value = '';

                                ws.send(message);
                                addMessageToList(message);
                            }}
                        }});

                        function addMessageToList(message) {{
                            const li = document.createElement('li');
                            li.innerText = message;
                            messages.appendChild(li);
                        }}
                    </script>
                </body>
            </html>
            "
            );

            tcp_stream
                .write_all(format!("HTTP/1.1 200 OK\r\n\r\n{}", index_html).as_bytes())
                .await?;
        }
        _ => {
            tcp_stream
                .write_all(b"HTTP/1.1 404 Not Found\r\n\r\n")
                .await?;
        }
    }

    Ok(())
}

async fn send_other_users_messages_to_user(
    tcp_write: &mut OwnedWriteHalf,
    rx: &mut tokio::sync::mpsc::Receiver<String>,
    my_id: u64,
    close_notify: Arc<tokio::sync::Notify>,
) -> Result<()> {
    // mpsc = multiple producer, single consumer queue

    /*
    다른 유저들이 보낸 메시지를 받아서
    내가 연결된 유저에게 보내주자.

    Q. 다른 유저들이 보낸 메시지를 내가 어떻게 받아?
    */

    // 클로즈 되었거나, 새 메시지를 받거나!

    loop {
        tokio::select! {
            _ = close_notify.notified() => {
                println!("user {my_id}: Connection Closed");
                rx.close();
                while let Some(message) = rx.recv().await {
                    write_text_message(tcp_write, &message).await?;
                }
                break;
            }
            Some(message) = rx.recv() => {
                write_text_message(tcp_write, &message).await?;
            }
        }
    }

    Ok(())
}

// 0                   1                   2                   3
// 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
// +-+-+-+-+-------+-+-------------+-------------------------------+
// |F|R|R|R| opcode|M| Payload len |    Extended payload length    |
// |I|S|S|S|  (4)  |A|     (7)     |             (16/64)           |
// |N|V|V|V|       |S|             |   (if payload len==126/127)   |
// | |1|2|3|       |K|             |                               |
// +-+-+-+-+-------+-+-------------+ - - - - - - - - - - - - - - - +
// |     Extended payload length continued, if payload len == 127  |
// + - - - - - - - - - - - - - - - +-------------------------------+
// |                               |Masking-key, if MASK set to 1  |
// +-------------------------------+-------------------------------+
// | Masking-key (continued)       |          Payload Data         |
// +-------------------------------- - - - - - - - - - - - - - - - +
// :                     Payload Data continued ...                :
// + - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - +
// |                     Payload Data continued ...                |
// +---------------------------------------------------------------+

struct PartialWebsocketMessage {
    core_payload_length: Option<u8>,
    extended_payload_length: Option<u64>,
    masking_key: Option<[u8; 4]>,
    payload: Option<Vec<u8>>,
}
impl PartialWebsocketMessage {
    fn clean_up(&mut self) {
        self.core_payload_length = None;
        self.extended_payload_length = None;
        self.masking_key = None;
        self.payload = None;
    }
}

enum ReceiveUserMessageError {
    Io(std::io::Error),
    NonSupported(String),
    Disconnected,
    FailToSaveMessageToDb,
}

async fn receive_user_message_and_send_to_other_users(
    tcp_read: &mut OwnedReadHalf,
    partial_message: &mut PartialWebsocketMessage,
    my_id: u64,
    user_txs: &UserTxs,
    db: &Db,
) -> Result<(), ReceiveUserMessageError> {
    /*
    Timeout동안 메시지를 유저로부터 기다려보고
    오면 그걸 다른 유저들에게 보내주고
    안오면 말구~
    */

    // 지난시간에 내가 어디까지 받았는지를 계산.

    if partial_message.core_payload_length.is_none() {
        let mut core_header = [0u8; 2];
        tcp_read
            .read_exact(&mut core_header)
            .await
            .map_err(ReceiveUserMessageError::Io)?;

        let fin = core_header[0] & 0b1000_0000 != 0;
        let opcode = core_header[0] & 0b0000_1111;
        let mask = core_header[1] & 0b1000_0000 != 0;

        if !fin {
            return Err(ReceiveUserMessageError::NonSupported(
                "Fragmented messages are not supported".to_string(),
            ));
        }
        match opcode {
            1 => {}
            8 => {
                return Err(ReceiveUserMessageError::Disconnected);
            }
            _ => {
                return Err(ReceiveUserMessageError::NonSupported(format!(
                    "Not supported opcode: {}",
                    opcode
                )));
            }
        }
        if !mask {
            return Err(ReceiveUserMessageError::NonSupported(
                "Unmasked messages are not supported".to_string(),
            ));
        }

        let core_payload_len = core_header[1] & 0b0111_1111;
        partial_message.core_payload_length = Some(core_payload_len);
    }

    let core_payload_len = partial_message.core_payload_length.unwrap();

    let total_payload_length = 'total_payload_length: {
        if let Some(extended_payload_length) = partial_message.extended_payload_length {
            break 'total_payload_length extended_payload_length;
        }

        let extended_payload_header_byte_length = if core_payload_len == 126 {
            2
        } else if core_payload_len == 127 {
            8
        } else {
            partial_message.extended_payload_length = Some(core_payload_len as u64);
            break 'total_payload_length core_payload_len as u64;
        };

        let mut extended_payload_header = vec![0u8; extended_payload_header_byte_length];

        tcp_read
            .read_exact(&mut extended_payload_header)
            .await
            .map_err(ReceiveUserMessageError::Io)?;

        let total_payload_length = match extended_payload_header_byte_length {
            2 => u16::from_be_bytes(extended_payload_header.try_into().unwrap()) as u64,
            8 => u64::from_be_bytes(extended_payload_header.try_into().unwrap()),
            _ => unreachable!(),
        };

        partial_message.extended_payload_length = Some(total_payload_length);
        total_payload_length
    };

    if partial_message.masking_key.is_none() {
        let mut masking_key = [0u8; 4];
        tcp_read
            .read_exact(&mut masking_key)
            .await
            .map_err(ReceiveUserMessageError::Io)?;

        partial_message.masking_key = Some(masking_key);
    }

    let masking_key = partial_message.masking_key.unwrap();

    if partial_message.payload.is_none() {
        let mut payload = vec![0u8; total_payload_length as usize];

        tcp_read
            .read_exact(&mut payload)
            .await // 여기 좀 문제가 될듯?
            .map_err(ReceiveUserMessageError::Io)?;

        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= masking_key[i % 4];
        }

        partial_message.payload = Some(payload);
    }

    let payload = partial_message.payload.take().unwrap();
    let text = String::from_utf8(payload).unwrap();

    db.add_message(&text)
        .await
        .map_err(|_| ReceiveUserMessageError::FailToSaveMessageToDb)?;
    send_to_other_users(text, my_id, user_txs).await;

    Ok(())

    // Q. 메시지가 오다가 중간에 말수도 있나요? 부분만 올 수 있나요?
    // A. 네, 왜냐하면 TCP는 스트림이기 때문에, 메시지가 한번에 올 수도 있고, 여러번에 나눠서 올 수도 있습니다.

    // Q. 그러면 내가 메시지 일부를 받아버렸음. 메시지를 파싱하다가 다 안와서 실패함. 그럼 함수를 끝낼건데, 그럼..... 읽었떤 메시지 어디로 감?
    // A. 사라집니다. 왜냐하면 함수의 스택 프레임이 사라지면서, 그 안에 있던 변수들도 사라지기 때문입니다.
}

async fn send_to_other_users(text: String, my_id: u64, user_txs: &UserTxs) {
    // RAII: Resource Acquisition Is Initialization
    let user_txs = user_txs.lock().await;
    let other_user_txs = user_txs.iter().filter(|user_tx| user_tx.id != my_id);

    // Q. user_txs에는 나를 포함해서 다 있는데, 나를 제외한 txs를 얻으려면 어떻게 해야합니까?
    // A. tx를 구분할 수 있는 그들만의 고유한 값이 있으면 되겠네! 그리고 내가 나의 tx의 고유값을 알고 있으면 되겠네!

    for user_tx in other_user_txs {
        let _ = user_tx.tx.send(text.clone()).await;
    }
}

async fn write_text_message(tcp_write: &mut OwnedWriteHalf, message: &str) -> Result<()> {
    let mut core_header = [0u8; 2];
    core_header[0] |= 0b1000_0001;

    if message.len() > 125 {
        // TODO
        return Err(anyhow::anyhow!("Message too long"));
    }
    core_header[1] |= message.len() as u8;

    tcp_write.write_all(&core_header).await?;
    tcp_write.write_all(message.as_bytes()).await?;

    Ok(())
}

fn generate_new_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_STREAM_ID: AtomicU64 = AtomicU64::new(0);

    NEXT_STREAM_ID.fetch_add(1, Ordering::Relaxed)
}
