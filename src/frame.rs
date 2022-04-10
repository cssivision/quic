use std::fmt;

use bytes::{Buf, BufMut, Bytes};

use crate::range_set::ArrayRangeSet;
use crate::varint;

#[derive(Debug)]
pub enum Frame {
    Padding,
    Ping,
    Ack(Ack),
}

pub struct Ack {
    pub largest: u64,
    pub delay: u64,
    pub additional: Bytes,
    pub ecn: Option<EcnCounts>,
}

impl fmt::Debug for Ack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unimplemented!()
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Type(u64);

impl Type {
    pub const ACK: Type = Type(0x02);
    pub const ACK_ECN: Type = Type(0x03);
}

impl From<Type> for u64 {
    fn from(x: Type) -> Self {
        x.0
    }
}

impl Ack {
    pub fn encode<W: BufMut>(
        delay: u64,
        ranges: &ArrayRangeSet,
        ecn: Option<&EcnCounts>,
        buf: &mut W,
    ) {
        let mut rest = ranges.iter().rev();
        let first = rest.next().unwrap();
        let largest = first.end - 1;
        let first_size = first.end - first.start;
        if ecn.is_some() {
            varint::encode(Type::ACK_ECN.into(), buf);
        } else {
            varint::encode(Type::ACK.into(), buf);
        }
        varint::encode(largest, buf);
        varint::encode(delay, buf);
        varint::encode(ranges.len() as u64 - 1, buf);
        // TODO why substract 1?
        varint::encode(first_size - 1, buf);

        let mut prev = first.start;
        for block in rest {
            let size = block.end - block.start;
            // TODO why substract 1?
            varint::encode(prev - block.end - 1, buf);
            // TODO why substract 1?
            varint::encode(size - 1, buf);
            prev = block.start;
        }
        if let Some(x) = ecn {
            x.encode(buf)
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct EcnCounts {
    pub ect0: u64,
    pub ect1: u64,
    pub ce: u64,
}

impl EcnCounts {
    pub fn encode<W: BufMut>(&self, buf: &mut W) {
        varint::encode(self.ect0, buf);
        varint::encode(self.ect1, buf);
        varint::encode(self.ce, buf);
    }
}
