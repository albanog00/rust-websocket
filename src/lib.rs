use base64::{prelude::BASE64_STANDARD, Engine};

mod connection;
mod frame;
mod handshake;

pub use connection::*;
pub use frame::*;
pub use handshake::*;
use rand::Rng;

pub fn generate_random_base64_str() -> String {
    let buf: [u8; 16] = rand::random();
    BASE64_STANDARD.encode(buf)
}

pub fn random_i32_to_u8_vec() -> Vec<u8> {
    let value: u32 = rand::thread_rng().gen();
    let mut arr = vec![0; 4];

    arr[0] = (value >> 24 & 0xff) as u8;
    arr[1] = (value >> 16 & 0xff) as u8;
    arr[2] = (value >> 8 & 0xff) as u8;
    arr[3] = (value & 0xff) as u8;

    arr
}

pub fn base64_encode<T>(s: T) -> String
where
    T: AsRef<[u8]>,
{
    BASE64_STANDARD.encode(s)
}
