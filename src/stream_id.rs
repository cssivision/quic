use std::fmt;

use crate::varint::VarInt;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct StreamId(u64);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum StreamType {
    ClientBidirectional,  // 0x00
    ServerBidirectional,  // 0x01
    ClientUnidirectional, // 0x02
    ServerUnidirectional, // 0x03
}

impl StreamId {
    /// Create a new StreamId
    pub fn new(st: StreamType, id: u64) -> Self {
        use StreamType::*;
        let bits = match st {
            ClientBidirectional => 0x00,
            ServerBidirectional => 0x01,
            ClientUnidirectional => 0x02,
            ServerUnidirectional => 0x03,
        };
        StreamId(id << 2 | bits)
    }

    pub fn id(&self) -> u64 {
        self.0 >> 2
    }

    pub fn stream_type(&self) -> StreamType {
        use StreamType::*;
        if self.0 & 0x2 == 0x00 {
            ClientBidirectional
        } else if self.0 & 0x2 == 0x01 {
            ServerBidirectional
        } else if self.0 & 0x2 == 0x02 {
            ClientUnidirectional
        } else if self.0 & 0x2 == 0x03 {
            ServerUnidirectional
        } else {
            unreachable!()
        }
    }
}

impl From<StreamId> for VarInt {
    fn from(x: StreamId) -> VarInt {
        unsafe { VarInt::from_u64_unchecked(x.0) }
    }
}

impl From<VarInt> for StreamId {
    fn from(v: VarInt) -> Self {
        Self(v.0)
    }
}

impl fmt::Display for StreamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use StreamType::*;
        let s = match self.stream_type() {
            ClientBidirectional => "client bidirectional",
            ServerBidirectional => "server bidirectional",
            ClientUnidirectional => "client unidirectional",
            ServerUnidirectional => "server unidirectional",
        };
        write!(f, "{} stream {}", s, self.id())
    }
}
