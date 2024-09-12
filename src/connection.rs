use std::{
    collections::HashMap,
    io::{Cursor, Error, ErrorKind},
};

use base64::{prelude::BASE64_STANDARD, Engine};
use frame::{Frame, StatusCode, Version};

use bytes::{Buf, BytesMut};
use sha1::{Digest, Sha1};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt, BufWriter},
    net::TcpStream,
};

use crate::frame::{self, HeaderMap, Opcode};

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

        match connection.read_frame().await.unwrap() {
            Some(Frame::HandshakeRequest { headers, .. }) => {
                let key = Self::handle_handshake(&headers)?;
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
                Ok(connection)
            }
            _ => Err(Error::new(
                ErrorKind::ConnectionRefused,
                "Invalid upgrade request",
            )),
        }
    }

    fn handle_handshake(headers: &HeaderMap) -> io::Result<String> {
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

    pub async fn write_frame(&mut self, frame: &Frame) -> io::Result<Option<()>> {
        match frame {
            Frame::HandshakeResponse {
                status_code,
                version,
                headers,
            } => {
                self.stream
                    .write_all(Version::parse(version).as_bytes())
                    .await?;
                self.stream.write_u8(b' ').await?;
                self.stream
                    .write_all(StatusCode::parse(status_code).as_bytes())
                    .await?;
                self.stream.write_all(b"\r\n").await?;

                for header in headers.iter() {
                    self.stream
                        .write_all(format!("{}: {}\r\n", header.0, header.1).as_bytes())
                        .await?
                }

                self.stream.write_all(b"\r\n").await?;
                self.stream.flush().await?;

                Ok(Some(()))
            }
            Frame::WebSocketResponse(response) => {
                self.stream
                    .write_u8(response.fin | Opcode::parse(&response.opcode))
                    .await?;

                let payload_len = response.payload.len();
                if payload_len <= 125 {
                    self.stream.write_u8(payload_len as u8).await?;
                } else if payload_len <= 1 << 16 {
                    self.stream.write_u8(126).await?;
                    self.stream.write_u16(payload_len as u16).await?;
                } else {
                    self.stream.write_u8(127).await?;
                    self.stream.write_u32(payload_len as u32).await?;
                }

                self.stream.write_all(&response.payload).await?;
                self.stream.flush().await?;

                match response.opcode {
                    Opcode::Close => {
                        println!("closing connection...");
                        self.close().await
                    }
                    _ => {}
                };

                Ok(Some(()))
            }
            _ => Ok(None),
        }
    }
}
