use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use connection::Connection;

use frame::{Frame, Opcode};
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
            println!("client {sock_addr} disconnected");
        });
    }
}

async fn handle_connection(_clients: &ClientMap, socket: TcpStream, _sock_addr: SocketAddr) {
    let mut connection = Connection::accept(socket).await.unwrap();

    let mut open = true;
    while open {
        if let Some(frame) = connection.read_frame().await.unwrap() {
            let response = match frame.opcode {
                Opcode::Text => Frame {
                    fin: true,
                    opcode: frame.opcode,
                    payload: "Hi client!".into(),
                },
                Opcode::Binary => Frame {
                    fin: true,
                    opcode: frame.opcode,
                    payload: "Hi client!".into(),
                },
                Opcode::Close => {
                    open = false;

                    Frame {
                        fin: true,
                        opcode: frame.opcode,
                        payload: [3, 232].into(),
                    }
                }
                Opcode::Ping => {
                    println!("Ping");

                    Frame {
                        fin: true,
                        opcode: Opcode::Pong,
                        payload: frame.payload,
                    }
                }
                Opcode::Pong => {
                    println!("Pong");
                    continue;
                }
                _ => continue,
            };

            connection.write_frame(&response).await.unwrap();
        } else {
            open = false;
        }
    }

    connection.close().await;
}
