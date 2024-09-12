use std::net::SocketAddr;

use connection::Connection;

use frame::{Frame, Opcode, WebSocketFrame};
use tokio::net::{TcpListener, TcpStream};

#[path = "../connection.rs"]
mod connection;

#[path = "../frame.rs"]
mod frame;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    loop {
        let (socket, sock_addr) = listener.accept().await.unwrap();

        tokio::spawn(async move {
            handle_connection(socket, sock_addr).await;
        });
    }
}

async fn handle_connection(socket: TcpStream, sock_addr: SocketAddr) {
    println!("handling {}", sock_addr);

    // let mut socket = socket;
    let mut connection = Connection::accept(socket).await.unwrap();

    while let Some(frame) = connection.read_frame().await.unwrap() {
        match frame {
            Frame::WebSocketRequest(request) => {
                let response = match request.opcode {
                    // frame::Opcode::Continuation => todo!(),
                    Opcode::Text => Frame::WebSocketResponse(WebSocketFrame {
                        fin: request.fin,
                        opcode: request.opcode,
                        masked: false,
                        masking_key: [0; 4],
                        payload: "Hi client!".into(),
                    }),
                    Opcode::Binary => Frame::WebSocketResponse(WebSocketFrame {
                        fin: request.fin,
                        opcode: request.opcode,
                        masked: false,
                        masking_key: [0; 4],
                        payload: "Hi client!".into(),
                    }),
                    Opcode::Close => Frame::WebSocketResponse(WebSocketFrame {
                        fin: request.fin,
                        opcode: request.opcode,
                        masked: false,
                        masking_key: [0; 4],
                        // payload: 1000_u16.to_be_bytes().into(),
                        payload: [3, 232].into(),
                    }),
                    Opcode::Ping => Frame::WebSocketResponse(WebSocketFrame {
                        fin: request.fin,
                        opcode: Opcode::Pong,
                        masked: false,
                        masking_key: request.masking_key,
                        payload: request.payload,
                    }),
                    Opcode::Pong => Frame::WebSocketResponse(WebSocketFrame {
                        fin: request.fin,
                        opcode: Opcode::Ping,
                        masked: false,
                        masking_key: request.masking_key,
                        payload: request.payload,
                    }),
                    _ => return,
                };

                connection.write_frame(&response).await.unwrap();
            }
            _ => return,
        };
    }
}
