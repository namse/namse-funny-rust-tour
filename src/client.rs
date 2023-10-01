use anyhow::Result;
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpStream,
};

fn main() -> Result<()> {
    let mut tcp_stream = TcpStream::connect("127.0.0.1:8080")?;
    println!("connected");
    tcp_stream.write_all([
        "GET / HTTP/1.1",
        "Host: localhost:8080",
        "Connection: Upgrade",
        "Pragma: no-cache",
        "Cache-Control: no-cache",
        "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/117.0.0.0 Safari/537.36",
        "Upgrade: websocket",
        "Origin: chrome://new-tab-page",
        "Sec-WebSocket-Version: 13",
        "Accept-Encoding: gzip, deflate, br",
        "Accept-Language: ko,en-US;q=0.9,en;q=0.8,ko-KR;q=0.7",
        "Sec-WebSocket-Key: dciI6oz+bLjVB+OMkBGPfA==",
        "Sec-WebSocket-Extensions: permessage-deflate; client_max_window_bits",
        "\r\n"
    ].join("\r\n").as_bytes())?;

    println!("sent");

    let lines = receive_http_request(&tcp_stream)?;
    println!("{:#?}", lines);

    Ok(())
}

fn receive_http_request(tcp_stream: &TcpStream) -> Result<String> {
    let mut buf_reader = BufReader::new(tcp_stream);
    let mut lines = vec![];

    for _ in 0..40 {
        let mut line = String::new();
        buf_reader.read_line(&mut line)?;
        println!("{}", line);

        lines.push(line);
    }

    Ok(lines.join(""))
}
