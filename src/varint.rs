use std::convert::TryInto;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct VarInt(u64);

impl VarInt {
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

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct VarIntExceeded;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct VarIntUnexpectedEof;
