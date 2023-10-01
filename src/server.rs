use anyhow::Result;
use base64::Engine;
use sha1::Digest;
use std::{
    io::{BufRead, BufReader, Read, Write},
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

    let (mut socket, _) = tcp_listener.accept()?;

    let request = receive_http_request(&socket)?;

    println!("{:#?}", request);

    if !request
        .headers
        .iter()
        .any(|(key, value)| key == "Upgrade" && value == "websocket")
    {
        println!("Not WebSocket Request");
        socket.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n")?;
        socket.shutdown(std::net::Shutdown::Both)?;
        return Ok(());
    }

    send_websocket_upgrade_response(&mut socket, &request)?;

    loop {
        let mut buffer = [0u8; 32 * 1024];
        let received = socket.read(&mut buffer)?;
        if received == 0 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }
        println!("received: {}", received);
        println!("{:?}", &buffer[..received]);
    }

    Ok(())
}

fn send_websocket_upgrade_response(socket: &mut TcpStream, request: &HttpRequest) -> Result<()> {
    let key = request
        .headers
        .iter()
        .find_map(|(key, value)| {
            if key == "Sec-WebSocket-Key" {
                Some(value)
            } else {
                None
            }
        })
        .ok_or(anyhow::anyhow!("Sec-WebSocket-Key not found"))?;

    let aceept_key = generate_accept_key(key);

    let extension = request.headers.iter().find_map(|(key, value)| {
        if key == "Sec-WebSocket-Extensions" {
            Some(value)
        } else {
            None
        }
    });

    let mut response = String::new();

    response.push_str("HTTP/1.1 101 Switching Protocols\r\n");
    response.push_str(&format!("Sec-WebSocket-Accept: {aceept_key}\r\n"));
    response.push_str("Connection: Upgrade\r\n");
    response.push_str("Upgrade: websocket\r\n");

    // if let Some(extension) = extension {
    //     response.push_str(&format!("Sec-WebSocket-Extensions: {}\r\n", extension));
    // }

    response.push_str("\r\n");

    println!("response: {}", response);
    socket.write_all(response.as_bytes())?;

    Ok(())
}

fn generate_accept_key(client_key: &str) -> String {
    let guid = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    let mut hasher = sha1::Sha1::new();
    hasher.update(client_key);
    hasher.update(guid);
    let result = hasher.finalize();

    base64::engine::general_purpose::STANDARD.encode(result)
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    protocol: String,
    headers: Vec<(String, String)>,
}

fn receive_http_request(tcp_stream: &TcpStream) -> Result<HttpRequest> {
    let mut buf_reader = BufReader::new(tcp_stream);

    let mut line = String::new();
    buf_reader.read_line(&mut line)?;

    let mut iter = line.split_whitespace();

    let method = iter.next().unwrap();
    let path = iter.next().unwrap();
    let protocol = iter.next().unwrap();

    let mut headers = Vec::new();

    loop {
        let mut line = String::new();
        buf_reader.read_line(&mut line)?;

        if line == "\r\n" {
            break;
        }

        let (key, value) = line.split_once(':').unwrap();

        headers.push((key.trim().to_string(), value.trim().to_string()));
    }

    Ok(HttpRequest {
        method: method.to_string(),
        path: path.to_string(),
        protocol: protocol.to_string(),
        headers,
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_generate_accept_key() {
        let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
        let accept_key = generate_accept_key(client_key);
        assert_eq!(accept_key, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }
}
