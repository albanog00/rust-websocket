use std::io::Cursor;

use frame::{Frame, StatusCode, Version};

use bytes::{Buf, BytesMut};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt, BufWriter},
    net::TcpStream,
};

use crate::frame::{self, Opcode};

//TODO: Implement Dispose
#[derive(Debug)]
pub struct Connection {
    stream: BufWriter<TcpStream>,
    buffer: BytesMut,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream: BufWriter::new(stream),
            buffer: BytesMut::with_capacity(4096),
        }
    }

    pub async fn read_frame(&mut self) -> io::Result<Option<Frame>> {
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

                println!("{:?}", self.stream.buffer());

                self.stream.flush().await?;

                Ok(Some(()))
            }
            _ => Ok(None),
        }
    }
}
