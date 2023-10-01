mod handshake;

use anyhow::Result;
use std::{
    io::{Read, Write},
    net::TcpStream,
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
    let mut chat_users = vec![];

    let (mut tcp_stream, _) = tcp_listener.accept()?;
    let stream_id = generate_new_id();

    if let Err(error) = handshake::handshake(&mut tcp_stream) {
        println!("Handshake Error: {:?}", error);
        return Ok(());
    }

    struct ChatUser {
        id: u64,
        tx: std::sync::mpsc::Sender<String>,
    }

    let (tx, rx) = std::sync::mpsc::channel();
    chat_users.push(ChatUser { id: stream_id, tx });

    // 우리가 만들려는 것: 채팅 서버
    // 받은 채팅을 남들에게 돌려줄 것.

    // 너 지금 immutable하게 빌리고 있는 중이지? 그렇다면 너 지금 이거 mutable하게 못빌려...!
    // 너 지금 읽기모드로 빌리고 있는 중이지? 그렇다면 너 지금 이거 쓰기모드로 못빌려...!

    // immutable = N
    // mutable = 1

    loop {
        // 데이터 받고
        let text_message_result = receive_text_message(&mut tcp_stream);
        let text = match text_message_result {
            Ok(text) => text,
            Err(_) => {
                chat_users.retain(|user| user.id != stream_id);
                break;
            }
        };

        chat_users
            .iter()
            // .filter(|user| user.id != stream_id)
            .for_each(|user| user.tx.send(text.clone()).unwrap());

        let message = rx.recv().unwrap();
        write_text_message(&mut tcp_stream, &message)?;
    }

    Ok(())
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
fn receive_text_message(tcp_stream: &mut TcpStream) -> Result<String> {
    let mut core_header = [0u8; 2];
    tcp_stream.read_exact(&mut core_header)?;

    let fin = core_header[0] & 0b1000_0000 != 0;
    assert!(fin, "Fragmented messages are not supported");

    let opcode = core_header[0] & 0b0000_1111;
    assert_eq!(opcode, 0b0001, "Only text messages are supported");

    let mask = core_header[1] & 0b1000_0000 != 0;
    assert!(mask, "Unmasked messages are not supported");

    let core_payload_len = core_header[1] & 0b0111_1111;

    let total_payload_length = 'total_payload_length: {
        let extended_payload_header_byte_length = if core_payload_len == 126 {
            2
        } else if core_payload_len == 127 {
            8
        } else {
            break 'total_payload_length core_payload_len as u64;
        };

        let mut extended_payload_header = vec![0u8; extended_payload_header_byte_length];

        tcp_stream.read_exact(&mut extended_payload_header)?;

        let extended_payload_length = match extended_payload_header_byte_length {
            2 => u16::from_be_bytes(extended_payload_header.try_into().unwrap()) as u64,
            8 => u64::from_be_bytes(extended_payload_header.try_into().unwrap()),
            _ => unreachable!(),
        };

        extended_payload_length
    };

    let mut masking_key = [0u8; 4];
    tcp_stream.read_exact(&mut masking_key)?;

    let mut payload = vec![0u8; total_payload_length as usize];

    tcp_stream.read_exact(&mut payload)?;

    for (i, byte) in payload.iter_mut().enumerate() {
        *byte ^= masking_key[i % 4];
    }

    Ok(String::from_utf8(payload)?)
}
