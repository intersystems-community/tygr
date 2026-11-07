//! Byte-set algebra: [`ByteSet`] types, built from [`Emp`], [`All`],
//! [`One`], [`Between`] and [`Union`].

use std::marker::PhantomData;

/// A type naming a constant byte set.
pub trait ByteSet {
    /// The set.
    #[doc(hidden)]
    const BYTE_SET: Type;
}

/// A 256-bit byte set.
#[doc(hidden)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Type([u64; 4]);

impl Type {
    /// No bytes.
    pub const EMP: Type = Type([0; 4]);
    /// Every byte.
    pub const ALL: Type = Type([u64::MAX; 4]);

    /// The bytes `lo..=hi`.
    pub const fn range(lo: u8, hi: u8) -> Type {
        let mut set = Type::EMP;
        let mut b = lo as u16;
        while b <= hi as u16 {
            set = set.with(b as u8);
            b += 1;
        }
        set
    }

    /// This set plus `b`.
    pub const fn with(self, b: u8) -> Type {
        let mut words = self.0;
        words[(b >> 6) as usize] |= 1 << (b & 63);
        Type(words)
    }

    /// Bytes in either set.
    pub const fn union(self, other: Type) -> Type {
        let (a, b) = (self.0, other.0);
        Type([a[0] | b[0], a[1] | b[1], a[2] | b[2], a[3] | b[3]])
    }

    /// Is `b` in the set?
    pub const fn contains(&self, b: u8) -> bool {
        self.0[(b >> 6) as usize] >> (b & 63) & 1 == 1
    }
}

/// No bytes.
pub struct Emp;
impl ByteSet for Emp {
    const BYTE_SET: Type = Type::EMP;
}

/// Every byte.
pub struct All;
impl ByteSet for All {
    const BYTE_SET: Type = Type::ALL;
}

/// Just byte `B`.
pub struct One<const B: u8>;
impl<const B: u8> ByteSet for One<B> {
    const BYTE_SET: Type = Type::EMP.with(B);
}

/// The bytes `LO..=HI`.
pub struct Between<const LO: u8, const HI: u8>;
impl<const LO: u8, const HI: u8> ByteSet for Between<LO, HI> {
    const BYTE_SET: Type = Type::range(LO, HI);
}

/// Bytes in `A` or `B`.
pub struct Union<A: ByteSet, B: ByteSet>(PhantomData<(A, B)>);
impl<A: ByteSet, B: ByteSet> ByteSet for Union<A, B> {
    const BYTE_SET: Type = A::BYTE_SET.union(B::BYTE_SET);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_set_ops() {
        let set = Type::EMP.with(0).with(63).with(64).with(255);
        for b in [0, 63, 64, 255] {
            assert!(set.contains(b));
        }
        assert!(!set.contains(1) && !set.contains(128));
        assert_eq!(Type::range(0, 255), Type::ALL);
        assert_eq!(
            Type::EMP.with(1).union(Type::EMP.with(2)),
            Type::range(1, 2)
        );
    }
}
