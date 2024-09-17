use bytes::Buf;
use std::io::Cursor;

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
    pub fn as_value(&self) -> u8 {
        match self {
            Self::Continuation => 0,
            Self::Text => 1,
            Self::Binary => 2,
            Self::Close => 8,
            Self::Ping => 9,
            Self::Pong => 10,
        }
    }

    pub fn parse(val: u8) -> Option<Self> {
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

        let opcode = match Opcode::parse(first & 0xf) {
            Some(val) => val,
            None => return None,
        };

        let second = cursor.get_u8();
        let masked = (second & 0x80) != 0;
        if !masked {
            return None;
        }

        let payload_len = match second & 0x7f {
            126 => cursor.get_u16() as usize,
            127 => cursor.get_u64() as usize,
            val => val as usize,
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

        println!("End of stream");

        Some(Self {
            fin,
            opcode,
            payload,
        })
    }

    pub fn encode(&mut self) -> Vec<u8> {
        let mut buf = Vec::<u8>::with_capacity(10 + self.payload.len());

        println!("Decoding frame: {:?}", self);

        buf.push((self.fin as u8) << 7 | self.opcode.as_value());

        let len = self.payload.len();
        if len <= 125 {
            buf.push(len as u8);
        } else if len <= 65535 {
            buf.push(126_u8);
            buf.push((len >> 8) as u8);
            buf.push((len & 0xff) as u8);
        } else {
            buf.push(127_u8);
            buf.push(((len >> 56) & 0xff) as u8);
            buf.push(((len >> 48) & 0xff) as u8);
            buf.push(((len >> 40) & 0xff) as u8);
            buf.push(((len >> 32) & 0xff) as u8);
            buf.push(((len >> 24) & 0xff) as u8);
            buf.push(((len >> 16) & 0xff) as u8);
            buf.push(((len >> 8) & 0xff) as u8);
            buf.push((len & 0xff) as u8);
        }

        buf.append(&mut self.payload);

        println!("Encoded frame: {:?}", buf);

        buf
    }
}
