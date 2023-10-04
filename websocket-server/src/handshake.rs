use anyhow::Result;
use base64::Engine;
use sha1::Digest;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};

pub(crate) async fn send_websocket_upgrade_response(
    tcp_stream: &mut TcpStream,
    request: &HttpRequest,
) -> Result<()> {
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

    let mut response = String::new();

    response.push_str("HTTP/1.1 101 Switching Protocols\r\n");
    response.push_str(&format!("Sec-WebSocket-Accept: {aceept_key}\r\n"));
    response.push_str("Connection: Upgrade\r\n");
    response.push_str("Upgrade: websocket\r\n");
    response.push_str("\r\n");

    tcp_stream.write_all(response.as_bytes()).await?;

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
pub(crate) struct HttpRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    _protocol: String,
    headers: Vec<(String, String)>,
}
impl HttpRequest {
    pub(crate) fn is_websocket_upgrade_request(&self) -> bool {
        self.headers
            .iter()
            .any(|(key, value)| key == "Upgrade" && value == "websocket")
    }
}

pub(crate) async fn receive_http_request(tcp_stream: &mut TcpStream) -> Result<HttpRequest> {
    let mut buf_reader = BufReader::new(tcp_stream);

    let mut line = String::new();
    buf_reader.read_line(&mut line).await?;

    let mut iter = line.split_whitespace();

    let method = iter.next().unwrap();
    let path = iter.next().unwrap();
    let protocol = iter.next().unwrap();

    let mut headers = Vec::new();

    loop {
        let mut line = String::new();
        buf_reader.read_line(&mut line).await?;

        if line == "\r\n" {
            break;
        }

        let (key, value) = line.split_once(':').unwrap();

        headers.push((key.trim().to_string(), value.trim().to_string()));
    }

    Ok(HttpRequest {
        method: method.to_string(),
        path: path.to_string(),
        _protocol: protocol.to_string(),
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
