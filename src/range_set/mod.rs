use std::ops::Range;

use tinyvec::TinyVec;

const ARRAY_RANGE_SET_INLINE_CAPACITY: usize = 2;

#[derive(Debug, Default)]
pub struct ArrayRangeSet(TinyVec<[Range<u64>; ARRAY_RANGE_SET_INLINE_CAPACITY]>);

impl ArrayRangeSet {
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = Range<u64>> + '_ {
        self.0.iter().cloned()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}
