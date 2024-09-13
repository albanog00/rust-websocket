use std::{
    collections::HashMap,
    io::{Cursor, Error, ErrorKind},
};

use bytes::{Buf, BytesMut};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt, BufWriter},
    net::TcpStream,
};

use crate::frame::{Frame, Handshake, HeaderMap, Opcode};

//TODO: Implement Dispose
#[derive(Debug)]
pub struct Connection {
    stream: BufWriter<TcpStream>,
    buffer: BytesMut,
    closed: bool,
}

impl Connection {
    fn new(stream: TcpStream) -> Self {
        Self {
            stream: BufWriter::new(stream),
            buffer: BytesMut::with_capacity(4096),
            closed: false,
        }
    }

    pub async fn accept(socket: TcpStream) -> io::Result<Self> {
        let mut connection = Self::new(socket);

        if let Some(request) = connection.read_handshake().await.unwrap() {
            let key = Handshake::try_key_handshake(&request.headers)?;
            let mut header_map = HashMap::new();

            header_map.insert("Upgrade".into(), "websocket".into());
            header_map.insert("Connection".into(), "Upgrade".into());
            header_map.insert("Sec-WebSocket-Accept".into(), key.into());

            connection
                .write_handshake(&Handshake {
                    header: "HTTP/1.1 101 Swithcing Protocols".into(),
                    headers: header_map,
                })
                .await
                .unwrap();

            return Ok(connection);
        }

        Err(Error::new(
            ErrorKind::ConnectionRefused,
            "Invalid upgrade request",
        ))
    }

    pub async fn read_handshake(&mut self) -> io::Result<Option<Handshake>> {
        if self.closed {
            return Ok(None);
        }

        loop {
            if 0 == self.stream.read_buf(&mut self.buffer).await? {
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionReset,
                        "connection reset by the peer",
                    ));
                }
            } else {
                let request = self.parse_handshake_request().await?;
                return Ok(Some(request));
            }
        }
    }

    async fn parse_handshake_request(&mut self) -> io::Result<Handshake> {
        let mut buf = Cursor::new(self.buffer.chunk());
        let request = Handshake::parse(&mut buf)?;

        let len = buf.position() as usize;
        buf.set_position(0);

        self.buffer.advance(len);

        Ok(request)
    }

    async fn write_handshake(&mut self, response: &Handshake) -> io::Result<()> {
        self.stream.write_all(response.header.as_slice()).await?;
        self.stream.write_all(b"\r\n").await?;

        for header in response.headers.iter() {
            self.stream
                .write_all(format!("{}: {}\r\n", header.0, header.1).as_bytes())
                .await?
        }

        self.stream.write_all(b"\r\n").await?;
        self.stream.flush().await?;

        Ok(())
    }

    pub async fn close(&mut self) {
        self.closed = true;
        self.stream.shutdown().await.unwrap();
    }

    pub async fn read_frame(&mut self) -> io::Result<Option<Frame>> {
        if self.closed {
            return Ok(None);
        }

        loop {
            if 0 == self.stream.read_buf(&mut self.buffer).await? {
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionReset,
                        "connection reset by the peer",
                    ));
                }
            } else {
                if let Some(frame) = self.parse_frame().await? {
                    return Ok(Some(frame));
                }
            }
        }
    }

    async fn parse_frame(&mut self) -> io::Result<Option<Frame>> {
        let mut buf = Cursor::new(self.buffer.chunk());

        if let Some(frame) = Frame::parse(&mut buf).await {
            let len = buf.position() as usize;
            buf.set_position(0);

            self.buffer.advance(len);
            return Ok(Some(frame));
        }

        Ok(None)
    }

    pub async fn write_frame(&mut self, frame: &Frame) -> io::Result<()> {
        self.stream
            .write_u8((frame.fin as u8) << 7 | Opcode::parse(&frame.opcode))
            .await?;

        let payload_len = frame.payload.len();
        if payload_len <= 125 {
            self.stream.write_u8(payload_len as u8).await?;
        } else if payload_len <= 1 << 16 {
            self.stream.write_u8(126).await?;
            self.stream.write_u16(payload_len as u16).await?;
        } else {
            self.stream.write_u8(127).await?;
            self.stream.write_u32(payload_len as u32).await?;
        }

        self.stream.write_all(&frame.payload).await?;
        self.stream.flush().await?;

        Ok(())
    }
}
