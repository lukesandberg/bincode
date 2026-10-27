//! Niches: first-byte values that a type's encoding never begins with.
//!
//! A wrapper type can use a niche of its payload in place of a separate tag. For example,
//! [`NonZeroU8`](core::num::NonZeroU8) never encodes as `0x00`, so `Option<NonZeroU8>` can encode
//! `None` as `0x00` and `Some(n)` as `n` alone, instead of a tag byte followed by the payload.
//!
//! This is opt-in: options only use niches with
//! [`Configuration::with_niches`](crate::config::Configuration::with_niches), and otherwise write
//! a tag as usual.
//!
//! ```
//! use core::num::NonZeroU8;
//!
//! let value = NonZeroU8::new(5);
//! let config = bincode::config::standard();
//! assert_eq!(bincode::encode_to_vec(value, config).unwrap(), [1, 5]);
//! assert_eq!(bincode::encode_to_vec(value, config.with_niches()).unwrap(), [5]);
//! ```
//!
//! Types declare their niches with [`Encode::ENCODE_NICHES`], [`Decode::DECODE_NICHES`] and
//! [`BorrowDecode::BORROW_DECODE_NICHES`], all of which default to [`NicheTable::NONE`].
//!
//! # Deriving
//!
//! Derived types declare niches automatically:
//!
//! - A struct takes the niches of its first encoded field, skipping fields with
//!   `#[bincode(skip)]`. If that field uses `with` or `with_serde`, or its type has a lifetime
//!   (like `&'a str` or `Cow<'a, str>`), the struct has no niches.
//! - An enum takes the variant indices it doesn't use, see [`NicheTable::variant_index`].
//!
//! ```
//! # #[cfg(feature = "derive")] {
//! use core::num::NonZeroU32;
//!
//! #[derive(bincode::Encode, bincode::Decode)]
//! struct UserId(NonZeroU32);
//!
//! #[derive(bincode::Encode, bincode::Decode)]
//! enum Status {
//!     Active,
//!     Suspended { days: u32 },
//! }
//!
//! let config = bincode::config::standard().with_niches();
//! let user = Some(UserId(NonZeroU32::new(7).unwrap()));
//! assert_eq!(bincode::encode_to_vec(user, config).unwrap(), [7]);
//! assert_eq!(bincode::encode_to_vec(None::<UserId>, config).unwrap(), [253]);
//! assert_eq!(bincode::encode_to_vec(None::<Status>, config).unwrap(), [2]);
//! # }
//! ```
//!
//! Add `#[bincode(no_niche)]` to a type to declare no niches. A struct needs it when its first
//! field leads back to the struct, like `next: Option<Box<Self>>`, since its niches would depend
//! on themselves; the compiler reports this as a cycle when evaluating `ENCODE_NICHES`.
//! Collections are fine: a `Vec`'s niches come from its length, not its elements.
//!
//! ```
//! # #[cfg(feature = "derive")] {
//! #[derive(bincode::Encode, bincode::Decode)]
//! #[bincode(no_niche)]
//! struct Node {
//!     next: Option<Box<Node>>,
//!     value: u8,
//! }
//!
//! #[derive(bincode::Encode, bincode::Decode)]
//! struct Tree {
//!     children: Vec<Tree>,
//!     value: u8,
//! }
//! # }
//! ```
//!
//! [`Encode::ENCODE_NICHES`]: crate::Encode::ENCODE_NICHES
//! [`Decode::DECODE_NICHES`]: crate::Decode::DECODE_NICHES
//! [`BorrowDecode::BORROW_DECODE_NICHES`]: crate::BorrowDecode::BORROW_DECODE_NICHES

use crate::config::{Config, Endianness, IntEncoding};

/// A contiguous range of first-byte values that a type's encoding never begins with.
///
/// A wrapper claims niches from the front of the range: `Option<T>` encodes `None` as
/// [`start`](Niches::start) and reports the rest, [`skip(1)`](Niches::skip), as its own niches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Niches {
    start: u8,
    len: u8,
}

impl Niches {
    /// No niches: every first byte may begin a valid encoding.
    pub const NONE: Niches = Niches { start: 0, len: 0 };

    /// The niches `start..=end`.
    ///
    /// # Panics
    ///
    /// Panics if `end < start`, or if the range covers every byte value (a type that writes at
    /// least one byte must begin with some valid byte). In a niche constant, this is a compile
    /// error:
    ///
    /// ```compile_fail
    /// use bincode::niche::{NicheTable, Niches};
    /// const ALL: NicheTable = NicheTable::uniform(Niches::range(0, u8::MAX));
    /// ```
    #[inline]
    pub const fn range(start: u8, end: u8) -> Niches {
        assert!(start <= end, "niche range must not be empty");
        assert!(
            !(start == 0 && end == u8::MAX),
            "niche range must not cover every byte value"
        );
        Niches {
            start,
            len: end - start + 1,
        }
    }

    /// The number of niches.
    #[inline]
    pub const fn len(self) -> u8 {
        self.len
    }

    /// Returns `true` if there are no niches.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    /// The first niche byte, which is the next one a wrapper should claim. Only meaningful when
    /// the range is not empty.
    #[inline]
    pub const fn start(self) -> u8 {
        self.start
    }

    /// Returns `true` if `byte` is one of the niches.
    #[inline]
    pub const fn contains(self, byte: u8) -> bool {
        byte >= self.start && byte - self.start < self.len
    }

    /// The niches left after a wrapper claims `n` of them from the front.
    #[inline]
    pub const fn skip(self, n: u8) -> Niches {
        if n >= self.len {
            Niches::NONE
        } else {
            Niches {
                start: self.start + n,
                len: self.len - n,
            }
        }
    }

    /// The niches of `Option<T>`, given the niches of `T`.
    ///
    /// If `T` has niches, `None` claims the first one and the rest pass through. Otherwise
    /// `Option<T>` writes a `0`/`1` tag, which leaves `2..=255` unused.
    #[inline]
    const fn option(self) -> Niches {
        if self.is_empty() {
            Niches::range(2, u8::MAX)
        } else {
            self.skip(1)
        }
    }
}

/// A type's [`Niches`] under each config that can change them.
///
/// Niches are declared as associated constants, which can't be generic over the config, so this
/// holds one entry for every config that encodes first bytes differently, and
/// [`get`](NicheTable::get) picks the entry for a given config. Under variable int encoding the
/// first byte of an integer is its tag, so endianness doesn't matter there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NicheTable {
    varint: Niches,
    fixint_little_endian: Niches,
    fixint_big_endian: Niches,
}

impl NicheTable {
    /// No niches under any config.
    pub const NONE: NicheTable = NicheTable::uniform(Niches::NONE);

    /// The same niches under every config, for types like `bool` whose encoding doesn't depend
    /// on it.
    #[inline]
    pub const fn uniform(niches: Niches) -> NicheTable {
        NicheTable {
            varint: niches,
            fixint_little_endian: niches,
            fixint_big_endian: niches,
        }
    }

    /// Niches under variable int encoding only, for types that start with a varint.
    #[inline]
    pub const fn varint_only(niches: Niches) -> NicheTable {
        NicheTable {
            varint: niches,
            fixint_little_endian: Niches::NONE,
            fixint_big_endian: Niches::NONE,
        }
    }

    /// Separate niches for variable int encoding and for fixed int encoding in each endianness.
    #[inline]
    pub const fn new(
        varint: Niches,
        fixint_little_endian: Niches,
        fixint_big_endian: Niches,
    ) -> NicheTable {
        NicheTable {
            varint,
            fixint_little_endian,
            fixint_big_endian,
        }
    }

    /// The niches of an enum with `variant_count` variants whose variant index is encoded as a
    /// `u32`, like `Result` or a derived enum.
    ///
    /// Under variable int encoding, the bytes from `variant_count` to 250 are unused, as are the
    /// tags of `u64` and wider. The `u16` and `u32` tags are not, since the decoder also accepts
    /// small values written with them, so this uses whichever of those two ranges is larger.
    /// Under fixed int encoding, the index is four bytes: in little endian its low byte comes
    /// first, and in big endian its first byte is zero.
    pub const fn variant_index(variant_count: u32) -> NicheTable {
        if variant_count == 0 {
            // An empty enum has no values to encode.
            return NicheTable::NONE;
        }
        let varint = if variant_count <= 248 {
            Niches::range(variant_count as u8, 250)
        } else {
            crate::varint::U32_NICHES
        };
        let fixint_little_endian = if variant_count <= 255 {
            Niches::range(variant_count as u8, u8::MAX)
        } else {
            Niches::NONE
        };
        let fixint_big_endian = if variant_count <= 1 << 24 {
            Niches::range(1, u8::MAX)
        } else {
            Niches::NONE
        };
        NicheTable::new(varint, fixint_little_endian, fixint_big_endian)
    }

    /// The niches under config `C`, or none unless `C` uses niches (see
    /// [`Configuration::with_niches`](crate::config::Configuration::with_niches)).
    #[inline]
    pub const fn get<C: Config>(self) -> Niches {
        if !C::USE_NICHES {
            return Niches::NONE;
        }
        match C::INT_ENCODING {
            IntEncoding::Variable => self.varint,
            IntEncoding::Fixed => match C::ENDIAN {
                Endianness::Little => self.fixint_little_endian,
                Endianness::Big => self.fixint_big_endian,
            },
        }
    }

    /// The niches of `Option<T>` under every config, given the niches of `T`.
    #[inline]
    pub(crate) const fn option(self) -> NicheTable {
        NicheTable {
            varint: self.varint.option(),
            fixint_little_endian: self.fixint_little_endian.option(),
            fixint_big_endian: self.fixint_big_endian.option(),
        }
    }
}

/// The niches of a slice or container length, which is encoded as a `u64`.
pub(crate) const SLICE_LEN_NICHES: NicheTable = NicheTable::varint_only(crate::varint::U64_NICHES);

#[test]
fn test_niches() {
    let n = Niches::range(253, 255);
    assert_eq!(n.len(), 3);
    assert_eq!(n.start(), 253);
    assert!(!n.contains(0));
    assert!(!n.contains(252));
    assert!(n.contains(253));
    assert!(n.contains(255));
    assert_eq!(n.skip(1), Niches::range(254, 255));
    assert_eq!(n.skip(3), Niches::NONE);
    assert_eq!(n.skip(200), Niches::NONE);

    assert!(Niches::NONE.is_empty());
    for b in 0..=u8::MAX {
        assert!(!Niches::NONE.contains(b));
    }

    let zero = Niches::range(0, 0);
    assert!(zero.contains(0));
    assert!(!zero.contains(1));

    assert_eq!(Niches::NONE.option(), Niches::range(2, 255));
    assert_eq!(zero.option(), Niches::NONE);
}

#[cfg(test)]
mod configs {
    use crate::config::{
        BigEndian, Configuration, Fixint, LittleEndian, NoLimit, UseNiches, Varint,
    };
    pub type VarintLe = Configuration<LittleEndian, Varint, NoLimit, UseNiches>;
    pub type VarintBe = Configuration<BigEndian, Varint, NoLimit, UseNiches>;
    pub type FixintLe = Configuration<LittleEndian, Fixint, NoLimit, UseNiches>;
    pub type FixintBe = Configuration<BigEndian, Fixint, NoLimit, UseNiches>;
}

#[test]
fn test_niche_table() {
    use configs::*;

    let table = NicheTable::new(
        Niches::range(250, 255),
        Niches::range(10, 255),
        Niches::range(1, 255),
    );
    assert_eq!(table.get::<VarintLe>(), Niches::range(250, 255));
    assert_eq!(table.get::<VarintBe>(), Niches::range(250, 255));
    assert_eq!(table.get::<FixintLe>(), Niches::range(10, 255));
    assert_eq!(table.get::<FixintBe>(), Niches::range(1, 255));
    // Configs that don't use niches never see any.
    assert_eq!(table.get::<crate::config::Configuration>(), Niches::NONE);

    let varint = NicheTable::varint_only(Niches::range(255, 255));
    assert_eq!(varint.get::<FixintLe>(), Niches::NONE);
    assert_eq!(varint.option().get::<FixintLe>(), Niches::range(2, 255));
    assert_eq!(varint.option().get::<VarintLe>(), Niches::NONE);
}

#[test]
fn test_variant_index() {
    use configs::{FixintBe, FixintLe, VarintLe as Varint};

    assert_eq!(NicheTable::variant_index(0), NicheTable::NONE);

    let two = NicheTable::variant_index(2);
    assert_eq!(two.get::<Varint>(), Niches::range(2, 250));
    assert_eq!(two.get::<FixintLe>(), Niches::range(2, 255));
    assert_eq!(two.get::<FixintBe>(), Niches::range(1, 255));

    // Past 248 variants, the tags of `u64` and wider leave more room than the bytes up to 250.
    assert_eq!(
        NicheTable::variant_index(248).get::<Varint>(),
        Niches::range(248, 250)
    );
    assert_eq!(
        NicheTable::variant_index(249).get::<Varint>(),
        crate::varint::U32_NICHES
    );
    assert_eq!(
        NicheTable::variant_index(1000).get::<Varint>(),
        crate::varint::U32_NICHES
    );

    assert_eq!(
        NicheTable::variant_index(255).get::<FixintLe>(),
        Niches::range(255, 255)
    );
    assert_eq!(
        NicheTable::variant_index(256).get::<FixintLe>(),
        Niches::NONE
    );
    assert_eq!(
        NicheTable::variant_index(1 << 24).get::<FixintBe>(),
        Niches::range(1, 255)
    );
    assert_eq!(
        NicheTable::variant_index((1 << 24) + 1).get::<FixintBe>(),
        Niches::NONE
    );
}
