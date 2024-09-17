use std::{
    collections::HashMap,
    io::{self, BufRead, Cursor},
};

use bytes::Buf;
use sha1::{Digest, Sha1};

use crate::base64_encode;

pub type HeaderMap = HashMap<String, String>;

#[derive(Debug)]
pub struct Handshake {
    pub header: Vec<u8>,
    pub headers: HeaderMap,
}

impl Handshake {
    pub fn parse(buf: &mut Cursor<&[u8]>) -> io::Result<Self> {
        let mut header = Vec::new();

        while header.is_empty() || header.last().unwrap() != &b'\n' {
            header.push(buf.get_u8());
        }

        let header_str = String::from_utf8(header.clone()).unwrap();
        let _header_parts: Vec<_> = header_str.split(" ").map(|x| x.trim()).collect();

        // let method = header_parts[0];
        // let uri = header_parts[1];
        // let version = header_parts[2];

        let lines: Vec<_> = buf.lines().map(|x| x.unwrap()).collect();

        let mut headers = HeaderMap::new();
        for line in lines.iter() {
            let parts: Vec<_> = line.split(": ").collect();

            if parts.len() == 2 {
                headers.insert(parts[0].into(), parts[1].into());
            }
        }

        Ok(Self { header, headers })
    }

    pub fn try_key_handshake(headers: &HeaderMap) -> io::Result<String> {
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
        let handshake_key = base64_encode(hasher.finalize());

        Ok(handshake_key)
    }

    pub fn encode(&mut self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.append(&mut self.header);
        buf.append(&mut b"\r\n".to_vec());

        for header in self.headers.iter() {
            buf.append(
                &mut format!("{}: {}\r\n\r\n", header.0, header.1)
                    .as_bytes()
                    .to_vec(),
            );
        }

        buf
    }
}
