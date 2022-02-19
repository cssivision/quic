use std::convert::{TryFrom, TryInto};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct VarInt(pub(crate) u64);

impl VarInt {
    pub unsafe fn from_u64_unchecked(x: u64) -> Self {
        VarInt(x)
    }

    pub fn from_u64(x: u64) -> Result<VarInt, VarIntExceeded> {
        if x < 2u64.pow(62) {
            Ok(VarInt(x))
        } else {
            Err(VarIntExceeded)
        }
    }

    pub fn len(self) -> usize {
        let x = self.0;
        if x < 2u64.pow(6) {
            1
        } else if x < 2u64.pow(14) {
            2
        } else if x < 2u64.pow(30) {
            4
        } else if x < 2u64.pow(62) {
            8
        } else {
            unreachable!()
        }
    }

    pub fn decode(b: &[u8]) -> Result<VarInt, VarIntUnexpectedEof> {
        if b.is_empty() {
            return Err(VarIntUnexpectedEof);
        }
        let mut buf = [0; 8];
        let t = b[0] >> 6;
        buf[0] = b[0];
        buf[0] &= 0b0011_1111;
        let x = match t {
            0b00 => u64::from(buf[0]),
            0b01 => {
                if b.len() < 2 {
                    return Err(VarIntUnexpectedEof);
                }
                buf.copy_from_slice(&b[1..2]);
                u64::from(u16::from_be_bytes(buf[..2].try_into().unwrap()))
            }
            0b10 => {
                if b.len() < 4 {
                    return Err(VarIntUnexpectedEof);
                }
                buf.copy_from_slice(&b[1..4]);
                u64::from(u32::from_be_bytes(buf[..4].try_into().unwrap()))
            }
            0b11 => {
                if b.len() < 8 {
                    return Err(VarIntUnexpectedEof);
                }
                buf.copy_from_slice(&b[1..8]);
                u64::from_be_bytes(buf)
            }
            _ => unreachable!(),
        };
        Ok(VarInt(x))
    }

    pub fn encode(&self) -> Vec<u8> {
        let x = self.0;
        if x < 2u64.pow(6) {
            (x as u8).to_be_bytes().to_vec()
        } else if x < 2u64.pow(14) {
            (0b01 << 14 | x as u16).to_be_bytes().to_vec()
        } else if x < 2u64.pow(30) {
            (0b10 << 30 | x as u32).to_be_bytes().to_vec()
        } else if x < 2u64.pow(62) {
            (0b11 << 62 | x).to_be_bytes().to_vec()
        } else {
            unreachable!()
        }
    }
}

impl From<VarInt> for u64 {
    fn from(x: VarInt) -> u64 {
        x.0
    }
}

impl From<u8> for VarInt {
    fn from(x: u8) -> Self {
        VarInt(x.into())
    }
}

impl From<u16> for VarInt {
    fn from(x: u16) -> Self {
        VarInt(x.into())
    }
}

impl From<u32> for VarInt {
    fn from(x: u32) -> Self {
        VarInt(x.into())
    }
}

impl TryFrom<u64> for VarInt {
    type Error = VarIntExceeded;
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        VarInt::from_u64(x)
    }
}

impl TryFrom<u128> for VarInt {
    type Error = VarIntExceeded;
    fn try_from(x: u128) -> Result<Self, Self::Error> {
        VarInt::from_u64(x.try_into().map_err(|_| VarIntExceeded)?)
    }
}

impl TryFrom<usize> for VarInt {
    type Error = VarIntExceeded;
    fn try_from(x: usize) -> Result<Self, Self::Error> {
        VarInt::try_from(x as u64)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct VarIntExceeded;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct VarIntUnexpectedEof;
