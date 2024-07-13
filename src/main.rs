// Uncomment this block to pass the first stage
use std::{
    error::Error,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    // Uncomment this block to pass the first stage

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("accepted new connection");
                handle_connection(stream);
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0; 256];
    let _n_read = stream.read(&mut buffer).unwrap();

    let request = std::str::from_utf8(&buffer).unwrap();
    let lines: Vec<_> = request.split("\r\n").collect();

    let [_method, path, _version]: [&str; 3] = lines[0]
        .split_whitespace()
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    let response = match path {
        "/" => "HTTP/1.1 200 OK\r\n\r\n".to_string(),
        p if p.starts_with("/echo/") => {
            let str = p.strip_prefix("/echo/").unwrap();
            let len = str.len();

            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {len}\r\n\r\n{str}"
            )
        }
        _ => "HTTP/1.1 404 Not Found\r\n\r\n".to_string(),
    };

    stream.write_all(response.as_bytes()).unwrap();
}
