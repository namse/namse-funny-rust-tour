mod handshake;

use anyhow::Result;
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

/*
오늘 무엇을 합니까?
저는 오늘 "Low-level WebSocket"을 할 것입니다.
뭔말이냐? 뭔말이긴 뭔말이야. 말 그대로입니다.
low level로 웹소켓을 구현하고, 그것으로 채팅을 구현하는 것.
= 라이브러리 안쓰겠다.
*/

fn main() -> Result<()> {
    let tcp_listener = std::net::TcpListener::bind("0.0.0.0:8080")?;

    let user_txs = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
    // Arc 쓰는 이유: 언제 힙에서 제거해야하는지 알기 위해서!

    loop {
        let (tcp_stream, _) = tcp_listener.accept()?;
        let (tx, rx) = std::sync::mpsc::channel();
        let id = generate_new_id();

        {
            let mut user_txs = user_txs.lock().unwrap();
            user_txs.push(UserTx { id, tx });
        }
        // Q. 코파일럿이 굳이 이거를 Block{}을 만들어서 위 코드를 짠 이유는?

        run_user_thread(tcp_stream, rx, id, user_txs.clone());
    }

    // Q. 유저 5천명 들어오면, 스레드 몇개? 5천개
}

struct UserTx {
    id: u64,
    tx: std::sync::mpsc::Sender<String>,
}

type UserTxs = std::sync::Arc<std::sync::Mutex<Vec<UserTx>>>;

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
fn run_user_thread(
    mut tcp_stream: TcpStream,
    rx: std::sync::mpsc::Receiver<String>,
    my_id: u64,
    user_txs: UserTxs,
) {
    std::thread::spawn(move || -> Result<()> {
        tcp_stream.set_read_timeout(Some(Duration::from_secs(1)))?;

        if let Err(error) = handshake::handshake(&mut tcp_stream) {
            println!("Handshake Error: {:?}", error);
            anyhow::bail!("Handshake Error: {:?}", error);
        }

        let mut partial_websocket_message = PartialWebsocketMessage {
            core_payload_length: None,
            extended_payload_length: None,
            masking_key: None,
            payload: None,
        };
        loop {
            match receive_user_message_and_send_to_other_users(
                &mut tcp_stream,
                &mut partial_websocket_message,
                my_id,
                &user_txs,
            ) {
                Ok(_) => {
                    partial_websocket_message.clean_up();
                }
                Err(error) => match error {
                    ReceiveUserMessageError::Io(error) => {
                        match error.kind() {
                            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
                                // 아무것도 안함.
                            }
                            _ => {
                                todo!("에러처리 해야함 {:?}", error.kind())
                            }
                        }
                    }
                    ReceiveUserMessageError::NonSupported(error) => {
                        todo!("에러처리 해야함 {:?}", error)
                    }
                },
            }

            match send_other_users_messages_to_user(&mut tcp_stream, &rx) {
                Ok(_) => {}
                Err(error) => {
                    todo!("에러처리 해야함 {:?}", error)
                }
            };
        }
    });
}

fn send_other_users_messages_to_user(
    tcp_stream: &mut TcpStream,
    rx: &std::sync::mpsc::Receiver<String>,
) -> Result<()> {
    // mpsc = multiple producer, single consumer queue

    /*
    다른 유저들이 보낸 메시지를 받아서
    내가 연결된 유저에게 보내주자.

    Q. 다른 유저들이 보낸 메시지를 내가 어떻게 받아?
    */

    // TODO: Non-blocking
    loop {
        let Ok(other_user_message) = rx.recv_timeout(Duration::from_secs(1)) else {
            return Ok(());
        };

        write_text_message(tcp_stream, &other_user_message)?;
    }
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
}

fn receive_user_message_and_send_to_other_users(
    tcp_stream: &mut TcpStream,
    partial_message: &mut PartialWebsocketMessage,
    my_id: u64,
    user_txs: &UserTxs,
) -> Result<(), ReceiveUserMessageError> {
    /*
    Timeout동안 메시지를 유저로부터 기다려보고
    오면 그걸 다른 유저들에게 보내주고
    안오면 말구~
    */

    // 지난시간에 내가 어디까지 받았는지를 계산.

    if partial_message.core_payload_length.is_none() {
        let mut core_header = [0u8; 2];
        tcp_stream
            .read_exact(&mut core_header)
            .map_err(ReceiveUserMessageError::Io)?;

        let fin = core_header[0] & 0b1000_0000 != 0;
        let opcode = core_header[0] & 0b0000_1111;
        let mask = core_header[1] & 0b1000_0000 != 0;

        println!("core_header: {:?}", core_header);
        println!("fin: {}, opcode: {}, mask: {}", fin, opcode, mask);

        if !fin {
            return Err(ReceiveUserMessageError::NonSupported(
                "Fragmented messages are not supported".to_string(),
            ));
        }
        if opcode != 0b0001 {
            return Err(ReceiveUserMessageError::NonSupported(
                "Only text messages are supported".to_string(),
            ));
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

        tcp_stream
            .read_exact(&mut extended_payload_header)
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
        tcp_stream
            .read_exact(&mut masking_key)
            .map_err(ReceiveUserMessageError::Io)?;

        partial_message.masking_key = Some(masking_key);
    }

    let masking_key = partial_message.masking_key.unwrap();

    if partial_message.payload.is_none() {
        let mut payload = vec![0u8; total_payload_length as usize];

        tcp_stream
            .read_exact(&mut payload) // 여기 좀 문제가 될듯?
            .map_err(ReceiveUserMessageError::Io)?;

        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= masking_key[i % 4];
        }

        partial_message.payload = Some(payload);
    }

    let payload = partial_message.payload.take().unwrap();
    let text = String::from_utf8(payload).unwrap();

    send_to_other_users(text, my_id, user_txs);

    Ok(())

    // Q. 메시지가 오다가 중간에 말수도 있나요? 부분만 올 수 있나요?
    // A. 네, 왜냐하면 TCP는 스트림이기 때문에, 메시지가 한번에 올 수도 있고, 여러번에 나눠서 올 수도 있습니다.

    // Q. 그러면 내가 메시지 일부를 받아버렸음. 메시지를 파싱하다가 다 안와서 실패함. 그럼 함수를 끝낼건데, 그럼..... 읽었떤 메시지 어디로 감?
    // A. 사라집니다. 왜냐하면 함수의 스택 프레임이 사라지면서, 그 안에 있던 변수들도 사라지기 때문입니다.
}

fn send_to_other_users(text: String, my_id: u64, user_txs: &UserTxs) {
    // RAII: Resource Acquisition Is Initialization
    let user_txs = user_txs.lock().unwrap();
    let other_user_txs = user_txs.iter().filter(|user_tx| user_tx.id != my_id);

    // Q. user_txs에는 나를 포함해서 다 있는데, 나를 제외한 txs를 얻으려면 어떻게 해야합니까?
    // A. tx를 구분할 수 있는 그들만의 고유한 값이 있으면 되겠네! 그리고 내가 나의 tx의 고유값을 알고 있으면 되겠네!

    for user_tx in other_user_txs {
        let _ = user_tx.tx.send(text.clone());
    }
}

fn write_text_message(tcp_stream: &mut TcpStream, message: &str) -> Result<()> {
    let mut core_header = [0u8; 2];
    core_header[0] |= 0b1000_0001;

    if message.len() > 125 {
        // TODO
        return Err(anyhow::anyhow!("Message too long"));
    }
    core_header[1] |= message.len() as u8;

    tcp_stream.write_all(&core_header)?;
    tcp_stream.write_all(message.as_bytes())?;

    Ok(())
}

fn generate_new_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_STREAM_ID: AtomicU64 = AtomicU64::new(0);

    NEXT_STREAM_ID.fetch_add(1, Ordering::Relaxed)
}
