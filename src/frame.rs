use std::{
    collections::HashMap,
    io::{BufRead as _, Cursor},
};

use tokio::io::AsyncReadExt;

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

pub enum Version {
    Http0_9,
    Http1_0,
    Http1_1,
    Http2_0,
}

impl Version {
    pub fn parse(&self) -> String {
        match self {
            Self::Http0_9 => "HTTP/0.9".into(),
            Self::Http1_0 => "HTTP/1.0".into(),
            Self::Http1_1 => "HTTP/1.1".into(),
            Self::Http2_0 => "HTTP/2.0".into(),
        }
    }

    pub fn compose(version: &str) -> Option<Self> {
        match version {
            "HTTP/0.9" => Some(Self::Http0_9),
            "HTTP/1.0" => Some(Self::Http1_0),
            "HTTP/1.1" => Some(Self::Http1_1),
            "HTTP/2.0" => Some(Self::Http2_0),
            _ => None,
        }
    }
}

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

pub type HeaderMap = HashMap<String, String>;

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
}

impl Frame {
    pub async fn parse(cursor: &mut Cursor<&[u8]>) -> Option<Self> {
        let mut buf = Vec::new();
        _ = cursor.read_to_end(&mut buf).await;

        /* GET */
        if buf[0..3].eq(&[0x47, 0x45, 0x54]) {
            if let Some(frame) = Self::parse_handshake_request(&mut buf) {
                return Some(frame);
            }
        }

        None
    }

    fn parse_handshake_request(buf: &Vec<u8>) -> Option<Self> {
        let method = Method::GET;
        let mut headers: HeaderMap = HashMap::new();

        let lines: Vec<_> = buf.lines().map(|x| x.unwrap()).collect();
        println!("reading request: {:?}", lines);

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
}
