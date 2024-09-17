use rust_websocket::{Connection, Frame, Opcode};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() {
    let client = TcpStream::connect("0.0.0.0:8080").await.unwrap();
    let mut connection = Connection::handshake(client).await.unwrap();

    connection
        .send_frame(&mut Frame {
            fin: true,
            opcode: Opcode::Ping,
            payload: [].into(),
        })
        .await
        .unwrap();

    if let Some(pong) = connection.read_frame().await.unwrap() {
        println!("Pong {:?}", pong);
    }

    connection
        .send_frame(&mut Frame {
            fin: true,
            opcode: Opcode::Close,
            payload: [3, 232].into(),
        })
        .await
        .unwrap();

    if let Some(close) = connection.read_frame().await.unwrap() {
        println!("Close {:?}", close);
    }
}
