use std::convert::{TryFrom, TryInto};

use bytes::{Buf, BufMut};

pub(crate) fn encode<B: BufMut>(x: u64, buf: &mut B) {
    unsafe { VarInt::from_u64_unchecked(x).encode(buf) };
}

pub(crate) fn decode<B: Buf>(b: &mut B) -> Result<VarInt, VarIntUnexpectedEof> {
    VarInt::decode(b)
}

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

    pub fn size(self) -> usize {
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

    pub fn decode<B: Buf>(b: &mut B) -> Result<VarInt, VarIntUnexpectedEof> {
        if !b.has_remaining() {
            return Err(VarIntUnexpectedEof);
        }
        let mut buf = [0; 8];
        buf[0] = b.get_u8();
        let t = buf[0] >> 6;
        buf[0] &= 0b0011_1111;
        let x = match t {
            0b00 => u64::from(buf[0]),
            0b01 => {
                if b.remaining() < 1 {
                    return Err(VarIntUnexpectedEof);
                }
                b.copy_to_slice(&mut buf[1..2]);
                u64::from(u16::from_be_bytes(buf[..2].try_into().unwrap()))
            }
            0b10 => {
                if b.remaining() < 3 {
                    return Err(VarIntUnexpectedEof);
                }
                b.copy_to_slice(&mut buf[1..4]);
                u64::from(u32::from_be_bytes(buf[..4].try_into().unwrap()))
            }
            0b11 => {
                if b.remaining() < 7 {
                    return Err(VarIntUnexpectedEof);
                }
                b.copy_to_slice(&mut buf[1..8]);
                u64::from_be_bytes(buf)
            }
            _ => unreachable!(),
        };
        Ok(VarInt(x))
    }

    pub fn encode<B: BufMut>(&self, buf: &mut B) {
        let x = self.0;
        if x < 2u64.pow(6) {
            buf.put_u8(x as u8);
        } else if x < 2u64.pow(14) {
            buf.put_u16(0b01 << 14 | x as u16);
        } else if x < 2u64.pow(30) {
            buf.put_u32(0b10 << 30 | x as u32);
        } else if x < 2u64.pow(62) {
            buf.put_u64(0b11 << 62 | x);
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
