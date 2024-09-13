use std::{
    collections::HashMap,
    io::{self, BufRead, Cursor},
};

use base64::{prelude::BASE64_STANDARD, Engine};
use bytes::Buf;
use sha1::{Digest, Sha1};

#[derive(Debug)]
pub enum Opcode {
    Continuation,
    Text,
    Binary,
    Close,
    Ping,
    Pong,
}

impl Opcode {
    pub fn parse(opcode: &Opcode) -> u8 {
        match opcode {
            Self::Continuation => 0,
            Self::Text => 1,
            Self::Binary => 2,
            Self::Close => 8,
            Self::Ping => 9,
            Self::Pong => 10,
        }
    }

    pub fn compose(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Continuation),
            1 => Some(Self::Text),
            2 => Some(Self::Binary),
            8 => Some(Self::Close),
            9 => Some(Self::Ping),
            10 => Some(Self::Pong),
            _ => None,
        }
    }
}

pub type HeaderMap = HashMap<String, String>;

pub struct Handshake {
    pub header: Vec<u8>,
    pub headers: HeaderMap,
}

impl Handshake {
    pub fn parse(buf: &mut Cursor<&[u8]>) -> io::Result<Self> {
        let mut header = Vec::new();

        while header.len() == 0 || header.last().unwrap() != &b'\n' {
            header.push(buf.get_u8());
        }

        let header_str = String::from_utf8(header.clone()).unwrap();
        let header_parts: Vec<_> = header_str.split(" ").map(|x| x.trim()).collect();

        let method = header_parts[0];
        if method != "GET" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "invalid method",
            ));
        }

        // let uri = header_parts[1];
        // let version = header_parts[1];

        let lines: Vec<_> = buf.lines().map(|x| x.unwrap()).collect();

        let mut headers: HeaderMap = HashMap::new();
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
        let handshake_key = BASE64_STANDARD.encode(hasher.finalize());

        Ok(handshake_key)
    }
}

#[derive(Debug)]
pub struct Frame {
    pub fin: bool,
    pub opcode: Opcode,
    pub payload: Vec<u8>,
}

impl Frame {
    pub async fn parse(cursor: &mut Cursor<&[u8]>) -> Option<Self> {
        let first = cursor.get_u8();
        let fin = first & 0x80 != 0;

        let opcode = match Opcode::compose(first & 0xf) {
            Some(val) => val,
            None => return None,
        };

        let second = cursor.get_u8();
        let masked = (second & 0x80) != 0;
        if !masked {
            return None;
        }

        let mut payload_len = (second & 0x7f) as usize;
        payload_len = match payload_len {
            126 => cursor.get_u16() as usize,
            127 => cursor.get_u64() as usize,
            _ => payload_len,
        };

        let masking_key = [
            cursor.get_u8(),
            cursor.get_u8(),
            cursor.get_u8(),
            cursor.get_u8(),
        ];

        let mut payload = vec![0; payload_len];
        for i in 0..payload_len {
            payload[i] = cursor.get_u8() ^ masking_key[i % 4];
        }

        Some(Self {
            fin,
            opcode,
            payload,
        })
    }
}
