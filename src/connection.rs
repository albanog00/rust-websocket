use std::{
    collections::HashMap,
    io::{Cursor, Error, ErrorKind},
};

use handshake::Handshake;
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    net::TcpStream,
};

use crate::frame::Frame;

//TODO: Implement Dispose
#[derive(Debug)]
pub struct Connection {
    reader: ReadHalf<TcpStream>,
    writer: WriteHalf<TcpStream>,
}

mod handshake;

impl Connection {
    fn new(stream: TcpStream) -> Self {
        let (reader, writer) = tokio::io::split(stream);

        Self { reader, writer }
    }

    pub async fn accept(socket: TcpStream) -> io::Result<Self> {
        let mut connection = Self::new(socket);

        if let Some(request) = connection.read_handshake().await.unwrap() {
            let key = Handshake::try_key_handshake(&request.headers)?;

            let mut header_map = HashMap::new();

            header_map.insert("Upgrade".into(), "websocket".into());
            header_map.insert("Connection".into(), "Upgrade".into());
            header_map.insert("Sec-WebSocket-Accept".into(), key);

            connection
                .send_handshake(&Handshake {
                    header: "HTTP/1.1 101 Swithcing Protocols".into(),
                    headers: header_map,
                })
                .await
                .unwrap();

            return Ok(connection);
        }

        connection.close().await;

        Err(Error::new(
            ErrorKind::ConnectionRefused,
            "Invalid upgrade request",
        ))
    }

    pub async fn read_handshake(&mut self) -> io::Result<Option<Handshake>> {
        let mut buf = Vec::with_capacity(8192);

        if 0 == self.reader.read_buf(&mut buf).await? {
            return Ok(None);
        }

        let request = self.parse_handshake(&mut buf).await?;
        Ok(Some(request))
    }

    async fn parse_handshake(&mut self, buf: &mut Vec<u8>) -> io::Result<Handshake> {
        let mut cursor = Cursor::new(buf.as_slice());
        let request = Handshake::parse(&mut cursor)?;

        Ok(request)
    }

    async fn send_handshake(&mut self, response: &Handshake) -> io::Result<()> {
        self.writer.write_all(response.header.as_slice()).await?;
        self.writer.write_all(b"\r\n").await?;

        for header in response.headers.iter() {
            self.writer
                .write_all(format!("{}: {}\r\n", header.0, header.1).as_bytes())
                .await?
        }

        self.writer.write_all(b"\r\n").await?;
        self.writer.flush().await?;

        Ok(())
    }

    pub async fn read_frame(&mut self) -> io::Result<Option<Frame>> {
        let mut buf = Vec::with_capacity(8192);

        if 0 == self.reader.read_buf(&mut buf).await? {
            return Ok(None);
        }

        if let Some(frame) = self.parse_frame(&mut buf).await? {
            return Ok(Some(frame));
        }

        Err(Error::new(ErrorKind::InvalidData, "Invalid frame"))
    }

    async fn parse_frame(&mut self, buf: &mut Vec<u8>) -> io::Result<Option<Frame>> {
        let mut buf = Cursor::new(buf.as_slice());

        if let Some(frame) = Frame::parse(&mut buf).await {
            return Ok(Some(frame));
        }

        Ok(None)
    }

    pub async fn send_frame(&mut self, frame: &mut Frame) -> io::Result<()> {
        self.writer.write_all(&frame.encode()).await?;
        self.writer.flush().await?;

        Ok(())
    }

    pub async fn close(&mut self) {
        self.writer.shutdown().await.unwrap();
    }
}
