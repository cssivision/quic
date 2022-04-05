use std::fmt;

use bytes::Bytes;

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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct EcnCounts {
    pub ect0: u64,
    pub ect1: u64,
    pub ce: u64,
}
