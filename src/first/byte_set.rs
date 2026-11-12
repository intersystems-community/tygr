//! Byte-set algebra: [`ByteSet`] types, built from [`Empty`], [`All`],
//! [`One`], [`Between`] and [`Union`].

use std::marker::PhantomData;

/// A type naming a constant byte set.
pub trait ByteSet {
    /// The set.
    #[doc(hidden)]
    const BYTE_SET: Set;
}

/// A 256-bit byte set.
#[doc(hidden)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Set([u64; 4]);

impl Set {
    /// No bytes.
    pub const EMPTY: Set = Set([0; 4]);
    /// Every byte.
    pub const ALL: Set = Set([u64::MAX; 4]);

    /// The bytes `lo..=hi`.
    pub const fn range(lo: u8, hi: u8) -> Set {
        let mut set = Set::EMPTY;
        let mut b = lo as u16;
        while b <= hi as u16 {
            set = set.with(b as u8);
            b += 1;
        }
        set
    }

    /// This set plus `b`.
    pub const fn with(self, b: u8) -> Set {
        let mut words = self.0;
        words[(b >> 6) as usize] |= 1 << (b & 63);
        Set(words)
    }

    /// Bytes in either set.
    pub const fn union(self, other: Set) -> Set {
        let (a, b) = (self.0, other.0);
        Set([a[0] | b[0], a[1] | b[1], a[2] | b[2], a[3] | b[3]])
    }

    /// Is `b` in the set?
    pub const fn contains(&self, b: u8) -> bool {
        self.0[(b >> 6) as usize] & (1 << (b & 63)) != 0
    }
}

/// No bytes.
pub struct Empty;
impl ByteSet for Empty {
    const BYTE_SET: Set = Set::EMPTY;
}

/// Every byte.
pub struct All;
impl ByteSet for All {
    const BYTE_SET: Set = Set::ALL;
}

/// Just byte `B`.
pub struct One<const B: u8>;
impl<const B: u8> ByteSet for One<B> {
    const BYTE_SET: Set = Set::EMPTY.with(B);
}

/// The bytes `LO..=HI`.
pub struct Between<const LO: u8, const HI: u8>;
impl<const LO: u8, const HI: u8> ByteSet for Between<LO, HI> {
    const BYTE_SET: Set = Set::range(LO, HI);
}

/// Bytes in `A` or `B`.
pub struct Union<A: ByteSet, B: ByteSet>(PhantomData<(A, B)>);
impl<A: ByteSet, B: ByteSet> ByteSet for Union<A, B> {
    const BYTE_SET: Set = A::BYTE_SET.union(B::BYTE_SET);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_set_ops() {
        let set = Set::EMPTY.with(0).with(63).with(64).with(255);
        for b in [0, 63, 64, 255] {
            assert!(set.contains(b));
        }
        assert!(!set.contains(1) && !set.contains(128));
        assert_eq!(Set::range(0, 255), Set::ALL);
        assert_eq!(
            Set::EMPTY.with(1).union(Set::EMPTY.with(2)),
            Set::range(1, 2)
        );
    }
}
