use std::{collections::HashMap, net::SocketAddr, pin::Pin, sync::Arc, thread, time::Duration};

use connection::Connection;

use frame::{Frame, Opcode, WebSocketFrame};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Mutex,
};

#[path = "../connection.rs"]
mod connection;

#[path = "../frame.rs"]
mod frame;

type ClientMap = Arc<Mutex<HashMap<SocketAddr, Arc<Mutex<Connection>>>>>;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    let clients = ClientMap::new(Mutex::new(HashMap::new()));

    loop {
        let (socket, sock_addr) = listener.accept().await.unwrap();
        let clients = clients.clone();

        tokio::spawn(async move {
            handle_connection(&clients, socket, sock_addr).await;

            println!("removing socket {}", sock_addr);
            clients.lock().await.remove(&sock_addr).unwrap();
            println!("{:?}", clients.lock().await);
        });
    }
}

async fn handle_connection(clients: &ClientMap, socket: TcpStream, sock_addr: SocketAddr) {
    let connection = Arc::new(Mutex::new(Connection::accept(socket).await.unwrap()));

    clients
        .lock()
        .await
        .insert(sock_addr.clone(), connection.clone());

    let mut close = false;
    while let Some(frame) = connection.lock().await.read_frame().await.unwrap() {
        match frame {
            Frame::WebSocketRequest(request) => {
                let response = match request.opcode {
                    // frame::Opcode::Continuation => todo!(),
                    Opcode::Text => Frame::WebSocketResponse(WebSocketFrame {
                        fin: 128,
                        opcode: request.opcode,
                        masked: false,
                        masking_key: [0; 4],
                        payload: "Hi client!".into(),
                    }),
                    Opcode::Binary => Frame::WebSocketResponse(WebSocketFrame {
                        fin: 128,
                        opcode: request.opcode,
                        masked: false,
                        masking_key: [0; 4],
                        payload: "Hi client!".into(),
                    }),
                    Opcode::Close => {
                        close = true;

                        Frame::WebSocketResponse(WebSocketFrame {
                            fin: 128,
                            opcode: request.opcode,
                            masked: false,
                            masking_key: [0; 4],
                            // payload: 1000_u16.to_be_bytes().into(),
                            payload: [3, 232].into(),
                        })
                    }
                    Opcode::Ping => {
                        println!("Ping");

                        Frame::WebSocketResponse(WebSocketFrame {
                            fin: 128,
                            opcode: Opcode::Pong,
                            masked: false,
                            masking_key: [0; 4],
                            payload: request.payload,
                        })
                    }
                    Opcode::Pong => {
                        println!("Pong");
                        continue;
                    }
                    _ => continue,
                };

                connection
                    .lock()
                    .await
                    .write_frame(&response)
                    .await
                    .unwrap();

                if close {
                    break;
                }
            }
            _ => break,
        };
    }

    println!("Closing connection for: {:?}", connection);
    connection.lock().await.close().await;
}
