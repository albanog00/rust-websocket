use std::{
    collections::HashMap,
    io::{BufRead, Cursor},
};

use tokio::io::AsyncReadExt;

#[derive(Debug)]
pub enum Method {
    GET,
    POST,
}

impl Method {
    pub fn parse(&self) -> String {
        match self {
            Method::GET => "GET".into(),
            Method::POST => "POST".into(),
        }
    }

    pub fn compose(method: &str) -> Option<Self> {
        match method {
            "GET" => Some(Self::GET),
            "POST" => Some(Self::POST),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum Version {
    // Http0_9,
    // Http1_0,
    Http1_1,
    // Http2_0,
}

impl Version {
    pub fn parse(&self) -> String {
        match self {
            // Self::Http0_9 => "HTTP/0.9".into(),
            // Self::Http1_0 => "HTTP/1.0".into(),
            Self::Http1_1 => "HTTP/1.1".into(),
            // Self::Http2_0 => "HTTP/2.0".into(),
        }
    }

    pub fn compose(version: &str) -> Option<Self> {
        match version {
            // "HTTP/0.9" => Some(Self::Http0_9),
            // "HTTP/1.0" => Some(Self::Http1_0),
            "HTTP/1.1" => Some(Self::Http1_1),
            // "HTTP/2.0" => Some(Self::Http2_0),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum StatusCode {
    SwitchingProtocols,
    OK,
    BadRequest,
    Forbidden,
    NotFound,
    InternalError,
}

impl StatusCode {
    pub fn parse(&self) -> String {
        match self {
            Self::SwitchingProtocols => "101 Switching Protocols".into(),
            Self::OK => "200 OK".into(),
            Self::BadRequest => "400 Bad Request".into(),
            Self::Forbidden => "403 Forbidden".into(),
            Self::NotFound => "404 Not Found".into(),
            Self::InternalError => "500 Internal Error".into(),
        }
    }

    pub fn compose(code: i32) -> Option<Self> {
        match code {
            200 => Some(Self::OK),
            400 => Some(Self::BadRequest),
            403 => Some(Self::Forbidden),
            404 => Some(Self::NotFound),
            500 => Some(Self::InternalError),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum DataFrame {
    Text,
    Binary,
}

impl DataFrame {
    pub fn parse(data_frame: &Self) -> u8 {
        match data_frame {
            Self::Text => 1,
            Self::Binary => 2,
        }
    }

    pub fn compose(val: u8) -> Option<Self> {
        match val {
            1 => Some(Self::Text),
            2 => Some(Self::Binary),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum ControlFrame {
    Close,
    Ping,
    Pong,
}

impl ControlFrame {
    pub fn parse(control_frame: &Self) -> u8 {
        match control_frame {
            Self::Close => 8,
            Self::Ping => 9,
            Self::Pong => 10,
        }
    }

    pub fn compose(val: u8) -> Option<Self> {
        match val {
            8 => Some(Self::Close),
            9 => Some(Self::Ping),
            10 => Some(Self::Pong),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum Opcode {
    Continuation,
    DataFrame(DataFrame),
    ControlFrame(ControlFrame),
}

impl Opcode {
    pub fn parse(opcode: &Opcode) -> u8 {
        match opcode {
            Opcode::Continuation => 0,
            Opcode::DataFrame(val) => DataFrame::parse(val),
            Opcode::ControlFrame(val) => ControlFrame::parse(val),
        }
    }

    pub fn compose(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Continuation),
            1 => Some(Self::DataFrame(DataFrame::Text)),
            2 => Some(Self::DataFrame(DataFrame::Binary)),
            8 => Some(Self::ControlFrame(ControlFrame::Close)),
            9 => Some(Self::ControlFrame(ControlFrame::Ping)),
            10 => Some(Self::ControlFrame(ControlFrame::Pong)),
            _ => None,
        }
    }
}

pub type HeaderMap = HashMap<String, String>;

#[derive(Debug)]
pub struct WebSocketFrame {
    pub fin: u8,
    pub opcode: Opcode,
    pub masked: bool,
    pub masking_key: [u8; 4],
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub enum Frame {
    HandshakeRequest {
        method: Method,
        uri: String,
        version: Version,
        headers: HeaderMap,
    },
    HandshakeResponse {
        status_code: StatusCode,
        version: Version,
        headers: HeaderMap,
    },
    WebSocketRequest(WebSocketFrame),
    WebSocketResponse(WebSocketFrame),
}

impl Frame {
    pub async fn parse(cursor: &mut Cursor<&[u8]>) -> Option<Self> {
        let mut buf = Vec::new();
        _ = cursor.read_to_end(&mut buf).await;

        /* GET */
        if buf[0..3].eq(&[0x47, 0x45, 0x54]) {
            return Self::parse_handshake_request(&mut buf);
        } else {
            println!("Web socket request: {:?}", buf);
            return Self::parse_websocket_frame(&mut buf);
        }
    }

    fn parse_handshake_request(buf: &Vec<u8>) -> Option<Self> {
        let method = Method::GET;
        let mut headers: HeaderMap = HashMap::new();

        let lines: Vec<_> = buf.lines().map(|x| x.unwrap()).collect();

        let request = &lines[0];
        let request_parts: Vec<_> = request.split(" ").collect();
        let uri = request_parts[1].into();
        let version = match Version::compose(request_parts[2]) {
            Some(v) => v,
            None => return None,
        };

        for line in lines.iter() {
            let parts: Vec<_> = line.split(": ").collect();

            if parts.len() == 2 {
                headers.insert(parts[0].into(), parts[1].into());
            }
        }

        Some(Self::HandshakeRequest {
            method,
            uri,
            version,
            headers,
        })
    }

    fn parse_websocket_frame(buf: &Vec<u8>) -> Option<Frame> {
        let mut idx: usize = 0;
        let fin = buf[idx] & 0x80;

        let opcode = match Opcode::compose(buf[idx] & 0xf) {
            Some(val) => val,
            None => return None,
        };
        idx += 1;

        let masked = (buf[idx] & 0x80) == 1;
        // if !masked {
        //     return None;
        // }

        let mut payload_len = (buf[idx] & 0x7f) as usize;
        idx += 1;

        payload_len = match payload_len {
            126 => {
                let val = read_u16_be(&buf[idx..idx + 1]) as usize;
                idx += 2;
                val
            }
            127 => {
                let val = read_u64_be(&buf[idx..idx + 7]) as usize;
                idx += 8;
                val
            }
            _ => payload_len,
        };

        let masking_key = [buf[idx], buf[idx + 1], buf[idx + 2], buf[idx + 3]];
        idx += 4;

        let mut payload = Vec::with_capacity(payload_len);
        for i in 0..payload_len {
            payload.push(buf[idx] ^ masking_key[i % 4]);
            idx += 1;
        }

        Some(Self::WebSocketRequest(WebSocketFrame {
            fin,
            opcode,
            masked,
            masking_key,
            payload,
        }))
    }
}

pub fn read_u16_be(buf: &[u8]) -> u16 {
    assert_eq!(buf.len(), 2);
    (buf[0] as u16) << 8 | buf[1] as u16
}

pub fn read_u64_be(buf: &[u8]) -> u64 {
    assert_eq!(buf.len(), 8);
    (buf[0] as u64) << 56
        | (buf[1] as u64) << 48
        | (buf[2] as u64) << 40
        | (buf[3] as u64) << 32
        | (buf[4] as u64) << 24
        | (buf[5] as u64) << 16
        | (buf[6] as u64) << 8
        | buf[7] as u64
}
