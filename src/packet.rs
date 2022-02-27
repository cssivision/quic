use std::fmt;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use thiserror::Error;

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

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
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

    pub(crate) fn from_buf(buf: &mut impl Buf, len: usize) -> Self {
        debug_assert!(len <= MAX_CONNECTION_ID_LENGTH);
        let mut res = ConnectionId {
            length: len as u8,
            data: [0; MAX_CONNECTION_ID_LENGTH],
        };
        buf.copy_to_slice(&mut res.data[..len]);
        res
    }

    /// Decode from long header format
    pub(crate) fn decode(buf: &mut impl Buf) -> Option<Self> {
        let length = buf.get_u8() as usize;
        if length > MAX_CONNECTION_ID_LENGTH || buf.remaining() < length {
            None
        } else {
            Some(ConnectionId::from_buf(buf, length))
        }
    }

    /// Encode in long header format
    pub(crate) fn encode(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.length as u8);
        buf.put_slice(&self.data[..self.length as usize]);
    }
}

impl fmt::Debug for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.data[0..self.length as usize].fmt(f)
    }
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.data.iter() {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum PacketNumber {
    U8(u8),
    U16(u16),
    U24(u32),
    U32(u32),
}

#[derive(Debug, Error, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum PacketDecodeError {
    #[error("unsupported version {version:x}")]
    UnsupportedVersion {
        src_cid: ConnectionId,
        dst_cid: ConnectionId,
        version: u32,
    },
    #[error("invalid header: {0}")]
    InvalidHeader(&'static str),
}

impl PacketNumber {
    pub(crate) fn new(n: u64, largest_acked: u64) -> Self {
        // From Appendix A
        // A.2. Sample Packet Number Encoding Algorithm
        // EncodePacketNumber(full_pn, largest_acked):
        //
        // // The number of bits must be at least one more
        // // than the base-2 logarithm of the number of contiguous
        // // unacknowledged packet numbers, including the new packet.
        // if largest_acked is None:
        //     num_unacked = full_pn + 1
        // else:
        //     num_unacked = full_pn - largest_acked
        //
        // min_bits = log(num_unacked, 2) + 1
        // num_bytes = ceil(min_bits / 8)
        //
        // // Encode the integer value and truncate to
        // // the num_bytes least significant bytes.
        // return encode(full_pn, num_bytes)
        let range = (n - largest_acked) * 2;
        if range < 1 << 8 {
            PacketNumber::U8(n as u8)
        } else if range < 1 << 16 {
            PacketNumber::U16(n as u16)
        } else if range < 1 << 24 {
            PacketNumber::U24(n as u32)
        } else if range < 1 << 32 {
            PacketNumber::U32(n as u32)
        } else {
            panic!("packet number too large to encode")
        }
    }

    pub(crate) fn len(self) -> usize {
        use self::PacketNumber::*;
        match self {
            U8(_) => 1,
            U16(_) => 2,
            U24(_) => 3,
            U32(_) => 4,
        }
    }

    pub(crate) fn encode<W: BufMut>(self, w: &mut W) {
        use self::PacketNumber::*;
        match self {
            U8(x) => w.put_u8(x),
            U16(x) => w.put_u16(x),
            U24(x) => w.put_uint(u64::from(x), 3),
            U32(x) => w.put_u32(x),
        }
    }

    pub(crate) fn decode<R: Buf>(len: usize, r: &mut R) -> Result<PacketNumber, PacketDecodeError> {
        use self::PacketNumber::*;
        let pn = match len {
            1 => U8(r.get_u8()),
            2 => U16(r.get_u16()),
            3 => U24(r.get_uint(3) as u32),
            4 => U32(r.get_u32()),
            _ => unreachable!(),
        };
        Ok(pn)
    }

    pub(crate) fn full(self, expected: u64) -> u64 {
        // From Appendix A
        // A.3. Sample Packet Number Decoding Algorithm
        // DecodePacketNumber(largest_pn, truncated_pn, pn_nbits):
        //    expected_pn  = largest_pn + 1
        //    pn_win       = 1 << pn_nbits
        //    pn_hwin      = pn_win / 2
        //    pn_mask      = pn_win - 1
        // The incoming packet number should be greater than
        // expected_pn - pn_hwin and less than or equal to
        // expected_pn + pn_hwin
        //
        // This means we cannot just strip the trailing bits from
        // expected_pn and add the truncated_pn because that might
        // yield a value outside the window.
        //
        // The following code calculates a candidate value and
        // makes sure it's within the packet number window.
        // Note the extra checks to prevent overflow and underflow.
        // candidate_pn = (expected_pn & ~pn_mask) | truncated_pn
        // if candidate_pn <= expected_pn - pn_hwin and
        //     candidate_pn < (1 << 62) - pn_win:
        //     return candidate_pn + pn_win
        // if candidate_pn > expected_pn + pn_hwin and
        //     candidate_pn >= pn_win:
        //     return candidate_pn - pn_win
        // return candidate_pn
        use self::PacketNumber::*;
        let truncated = match self {
            U8(x) => u64::from(x),
            U16(x) => u64::from(x),
            U24(x) => u64::from(x),
            U32(x) => u64::from(x),
        };
        let nbits = self.len() * 8;
        let win = 1 << nbits;
        let hwin = win / 2;
        let mask = win - 1;
        let candidate = (expected & !mask) | truncated;
        if expected.checked_sub(hwin).map_or(false, |x| candidate <= x) {
            candidate + win
        } else if candidate > expected + hwin && candidate > win {
            candidate - win
        } else {
            candidate
        }
    }
}

pub struct Packet {
    header: Header,
    version: u32,
    dst_cid: ConnectionId,
    src_cid: ConnectionId,
    number: PacketNumber,
    payload: BytesMut,
}

#[derive(Debug, Clone)]
pub enum Header {
    Initial {
        token: Bytes,
    },
    ZeroRTT,
    Handshake,
    Retry {
        token: Bytes,
        integrity_tag: [u8; 128],
    },
    VersionNegotiate,
}
