use bytes::{Bytes, BytesMut};

const LONG_HEADER_FORM: u8 = 0b1000_0000;
const LONG_HEADER_FIXED_BIT: u8 = 0b0100_0000;
const LONG_RESERVED_BITS: u8 = 0x0c;
const MAX_CONNECTION_ID_LENGTH: usize = 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LongHeaderType {
    Initial,   // 0x00
    ZeroRTT,   // 0x01
    Handshake, // 0x02
    Retry,     // 0x03
}

impl From<LongHeaderType> for u8 {
    fn from(t: LongHeaderType) -> u8 {
        use LongHeaderType::*;
        match t {
            Initial => LONG_HEADER_FORM | LONG_HEADER_FIXED_BIT,
            ZeroRTT => LONG_HEADER_FORM | LONG_HEADER_FIXED_BIT | (0x1 << 4),
            Handshake => LONG_HEADER_FORM | LONG_HEADER_FIXED_BIT | (0x2 << 4),
            Retry => LONG_HEADER_FORM | LONG_HEADER_FIXED_BIT | (0x3 << 4),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectionId {
    /// length of ConnectionId
    length: u8,
    /// bytes of ConnectionId
    data: [u8; MAX_CONNECTION_ID_LENGTH],
}

impl ConnectionId {
    pub fn new(data: &[u8]) -> Self {
        debug_assert!(data.len() <= MAX_CONNECTION_ID_LENGTH);
        let mut cid = Self {
            length: data.len() as u8,
            data: [0; MAX_CONNECTION_ID_LENGTH],
        };
        cid.data[..data.len()].copy_from_slice(data);
        cid
    }
}

pub struct PacketNumber(u32);

impl PacketNumber {}

pub struct Packet {
    pub header: HeaderType,
    pub version: u32,
    pub dst_cid: ConnectionId,
    pub src_cid: ConnectionId,
    pub number: PacketNumber,
    pub payload: BytesMut,
}

pub enum HeaderType {
    Initial {
        token: Bytes,
    },
    ZeroRTT,
    Handshake,
    Retry {
        token: Bytes,
        integrity_tag: [u8; 128],
    },
}
