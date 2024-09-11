use std::collections::{HashMap, HashSet};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use connection::Connection;

use base64::{prelude::BASE64_STANDARD, Engine};
use frame::{Frame, HeaderMap, StatusCode, Version, WebSocketFrame};
use sha1::{Digest, Sha1};
use tokio::io::{self, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[path = "../connection.rs"]
mod connection;

#[path = "../frame.rs"]
mod frame;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    let clients: Arc<Mutex<HashSet<SocketAddr>>> = Arc::new(Mutex::new(HashSet::new()));

    loop {
        let (socket, sock_addr) = listener.accept().await.unwrap();
        let clients = clients.clone();

        tokio::spawn(async move {
            handle_connection(socket, sock_addr, &clients).await;
        });
    }
}

async fn handle_connection(
    socket: TcpStream,
    sock_addr: SocketAddr,
    clients: &Arc<Mutex<HashSet<SocketAddr>>>,
) {
    {
        println!("handling {}", sock_addr);

        let mut socket = socket;

        {
            let mut clients = clients.lock().unwrap();

            if clients.contains(&sock_addr) {
                clients.remove(&sock_addr);
                _ = socket.shutdown();
            } else {
                clients.insert(sock_addr);
            }
        }

        let mut connection = Connection::new(socket);
        while let Some(frame) = connection.read_frame().await.unwrap() {
            match frame {
                Frame::HandshakeRequest { headers, .. } => match handle_handshake(headers) {
                    Ok(key) => {
                        let mut header_map = HashMap::new();

                        header_map.insert("Upgrade".into(), "websocket".into());
                        header_map.insert("Connection".into(), "Upgrade".into());
                        header_map.insert("Sec-WebSocket-Accept".into(), key.into());

                        let response = Frame::HandshakeResponse {
                            status_code: StatusCode::SwitchingProtocols,
                            version: Version::Http1_1,
                            headers: header_map,
                        };

                        connection.write_frame(&response).await.unwrap();
                    }
                    Err(_) => todo!(),
                },
                Frame::WebSocketRequest(request) => {
                    let response = Frame::WebSocketResponse(WebSocketFrame {
                        fin: request.fin,
                        opcode: request.opcode,
                        masked: false,
                        masking_key: [0; 4],
                        payload: "Hi client!".into(),
                    });

                    connection.write_frame(&response).await.unwrap();
                }
                _ => return,
            };
        }
    }
}

fn handle_handshake(headers: HeaderMap) -> io::Result<String> {
    const MAGIC: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    let key = match headers.get("Sec-WebSocket-Key") {
        Some(key) => key.to_owned(),
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "handshake failed with client",
            ))
        }
    };

    let mut hasher = Sha1::new();
    hasher.update(key);
    hasher.update(MAGIC);
    let handshake_key = BASE64_STANDARD.encode(hasher.finalize());

    Ok(handshake_key)
}
