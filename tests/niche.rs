#![cfg(feature = "std")]

use bincode::{
    config::{self, Config},
    error::DecodeError,
    niche::{NicheTable, Niches},
    BorrowDecode, Decode, Encode,
};
use core::{
    cell::{Cell, RefCell},
    cmp::Reverse,
    fmt::Debug,
    num::{NonZeroI32, NonZeroI8, NonZeroU32, NonZeroU64, NonZeroU8, Wrapping},
    ops::{Bound, Range, RangeInclusive},
    sync::atomic::{AtomicBool, AtomicU32},
    time::Duration,
};
use std::{
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, VecDeque},
    ffi::CString,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
};

/// Encodes `value` with `config`, checks the bytes, and decodes them back through the slice,
/// borrowing and `std::io::Read` paths. `std::io::Read` readers can't peek, so the last one shows
/// that decoding doesn't rely on lookahead.
fn check_with_config<T, C>(value: &T, expected: &[u8], config: C)
where
    T: Encode + Decode<()> + for<'de> BorrowDecode<'de, ()> + PartialEq + Debug,
    C: Config,
{
    let bytes = bincode::encode_to_vec(value, config).unwrap();
    assert_eq!(
        bytes,
        expected,
        "{value:?}: {} with {}",
        core::any::type_name::<T>(),
        core::any::type_name::<C>()
    );

    let (decoded, len): (T, usize) = bincode::decode_from_slice(&bytes, config).unwrap();
    assert_eq!(&decoded, value);
    assert_eq!(len, bytes.len());

    let (decoded, len): (T, usize) = bincode::borrow_decode_from_slice(&bytes, config).unwrap();
    assert_eq!(&decoded, value);
    assert_eq!(len, bytes.len());

    let decoded: T = bincode::decode_from_std_read(&mut &bytes[..], config).unwrap();
    assert_eq!(&decoded, value);
}

/// Runs [check_with_config] under every combination of endianness and int encoding. The niches
/// covered here don't depend on the config.
fn check<T>(value: T, expected: &[u8])
where
    T: Encode + Decode<()> + for<'de> BorrowDecode<'de, ()> + PartialEq + Debug,
{
    check_with_config(&value, expected, config::standard().with_niches());
    check_with_config(
        &value,
        expected,
        config::standard().with_niches().with_big_endian(),
    );
    check_with_config(&value, expected, config::legacy().with_niches());
    check_with_config(
        &value,
        expected,
        config::legacy().with_niches().with_big_endian(),
    );
}

/// Runs [check_with_config] under variable int encoding, in both endiannesses.
fn check_varint<T>(value: T, expected: &[u8])
where
    T: Encode + Decode<()> + for<'de> BorrowDecode<'de, ()> + PartialEq + Debug,
{
    check_with_config(&value, expected, config::standard().with_niches());
    check_with_config(
        &value,
        expected,
        config::standard().with_niches().with_big_endian(),
    );
}

#[test]
fn option_non_zero_is_one_byte() {
    check(None::<NonZeroU8>, &[0]);
    for n in 1..=u8::MAX {
        check(NonZeroU8::new(n), &[n]);
        check(NonZeroI8::new(n as i8), &[n]);
    }
}

#[test]
fn option_bool() {
    check(Some(false), &[0]);
    check(Some(true), &[1]);
    check(None::<bool>, &[2]);
}

#[test]
fn nested_options_claim_successive_niches() {
    check(Some(Some(true)), &[1]);
    check(Some(None::<bool>), &[2]);
    check(None::<Option<bool>>, &[3]);
    check(Some(Some(None::<bool>)), &[2]);
    check(Some(None::<Option<bool>>), &[3]);
    check(None::<Option<Option<bool>>>, &[4]);
}

#[test]
fn option_char() {
    check(Some('a'), b"a");
    check(Some('é'), "é".as_bytes());
    check(Some('🦀'), "🦀".as_bytes());
    check(None::<char>, &[0x80]);
}

#[test]
fn options_without_payload_niches_keep_their_tag() {
    check(Some(5u8), &[1, 5]);
    check(None::<u8>, &[0]);

    // The tag leaves 2..=255 unused, so an outer option needs no tag of its own.
    check(Some(Some(5u8)), &[1, 5]);
    check(Some(None::<u8>), &[0]);
    check(None::<Option<u8>>, &[2]);

    // `Option<NonZeroU8>` uses up the only niche, so the outer option falls back to a tag.
    check(Some(NonZeroU8::new(5)), &[1, 5]);
    check(Some(None::<NonZeroU8>), &[1, 0]);
    check(None::<Option<NonZeroU8>>, &[0]);
}

#[test]
fn wrappers_forward_niches() {
    check(None::<Box<bool>>, &[2]);
    check(Some(Box::new(true)), &[1]);
    check(None::<Rc<NonZeroU8>>, &[0]);
    check(Some(Rc::new(NonZeroU8::MAX)), &[255]);
    check(None::<Arc<NonZeroU8>>, &[0]);
    check(None::<Wrapping<bool>>, &[2]);
    check(None::<Reverse<bool>>, &[2]);
    check(None::<Cell<bool>>, &[2]);
    check(Some(Cell::new(true)), &[1]);
    check(None::<RefCell<bool>>, &[2]);

    assert_eq!(
        bincode::encode_to_vec(None::<&bool>, config::standard().with_niches()).unwrap(),
        [2]
    );

    let bytes =
        bincode::encode_to_vec(None::<Mutex<bool>>, config::standard().with_niches()).unwrap();
    assert_eq!(bytes, [2]);
    let (decoded, _): (Option<Mutex<bool>>, usize) =
        bincode::decode_from_slice(&[1], config::standard().with_niches()).unwrap();
    assert!(*decoded.unwrap().lock().unwrap());
}

#[test]
fn option_varint_uses_spare_tags() {
    // A varint never begins with the tag of a wider integer, or with the reserved 255.
    check_varint(None::<u16>, &[252]);
    check_varint(None::<u32>, &[253]);
    check_varint(None::<u64>, &[254]);
    check_varint(None::<usize>, &[254]);
    check_varint(None::<u128>, &[255]);
    check_varint(None::<i32>, &[253]);
    check_varint(None::<NonZeroU32>, &[253]);

    check_varint(Some(7u32), &[7]);
    check_varint(Some(-1i32), &[1]);
    check_varint(NonZeroU32::new(5), &[5]);
    check_varint(NonZeroI32::new(-1), &[1]);
    check_with_config(
        &Some(300u32),
        &[251, 44, 1],
        config::standard().with_niches(),
    );
    check_with_config(
        &Some(300u32),
        &[251, 1, 44],
        config::standard().with_niches().with_big_endian(),
    );
    check_with_config(
        &Some(u64::MAX),
        &[253, 255, 255, 255, 255, 255, 255, 255, 255],
        config::standard().with_niches(),
    );

    // Nested options take the next spare tag.
    check_varint(Some(Some(7u32)), &[7]);
    check_varint(Some(None::<u32>), &[253]);
    check_varint(None::<Option<u32>>, &[254]);
    check_varint(None::<Option<Option<u32>>>, &[255]);
    // `u128` has a single spare tag, so the outer option falls back to a tag.
    check_varint(Some(None::<u128>), &[1, 255]);
}

#[test]
fn option_fixint_keeps_its_tag() {
    // Every first byte is valid for a fixed-width integer or length.
    check_with_config(
        &Some(7u32),
        &[1, 7, 0, 0, 0],
        config::legacy().with_niches(),
    );
    check_with_config(
        &Some(7u32),
        &[1, 0, 0, 0, 7],
        config::legacy().with_niches().with_big_endian(),
    );
    check_with_config(&None::<u32>, &[0], config::legacy().with_niches());
    check_with_config(
        &Some(String::from("hi")),
        &[1, 2, 0, 0, 0, 0, 0, 0, 0, b'h', b'i'],
        config::legacy().with_niches(),
    );
}

#[test]
fn option_length_prefixed_uses_spare_tags() {
    check_varint(Some(String::from("hi")), &[2, b'h', b'i']);
    check_varint(Some(String::new()), &[0]);
    check_varint(None::<String>, &[254]);
    check_varint(Some(Box::<str>::from("hi")), &[2, b'h', b'i']);
    check_varint(Some(Rc::<str>::from("hi")), &[2, b'h', b'i']);
    check_varint(Some(Arc::<str>::from("hi")), &[2, b'h', b'i']);
    check_varint(Some(vec![true, false]), &[2, 1, 0]);
    check_varint(None::<Vec<u8>>, &[254]);
    check_varint(Some(Box::<[u8]>::from([7u8])), &[1, 7]);
    check_varint(Some(Rc::<[u8]>::from([7u8])), &[1, 7]);
    check_varint(Some(Arc::<[u8]>::from([7u8])), &[1, 7]);
    check_varint(Some(VecDeque::from([7u8])), &[1, 7]);
    check_varint(Some(BTreeMap::from([(1u8, 2u8)])), &[1, 1, 2]);
    check_varint(Some(BTreeSet::from([7u8])), &[1, 7]);
    check_varint(Some(HashMap::<u8, u8>::from([(1, 2)])), &[1, 1, 2]);
    check_varint(Some(HashSet::<u8>::from([7])), &[1, 7]);
    check_varint(Some(CString::new("a").unwrap()), &[1, b'a']);
    check_varint(Some(PathBuf::from("a")), &[1, b'a']);
    check_varint(Some(Some(String::new())), &[0]);
    check_varint(Some(None::<String>), &[254]);
    check_varint(None::<Option<String>>, &[255]);

    // `BinaryHeap` isn't `PartialEq`, so check it by hand.
    let heap = Some(BinaryHeap::from([7u8]));
    let bytes = bincode::encode_to_vec(&heap, config::standard().with_niches()).unwrap();
    assert_eq!(bytes, [1, 7]);
    let (decoded, _): (Option<BinaryHeap<u8>>, usize) =
        bincode::decode_from_std_read(&mut &bytes[..], config::standard().with_niches())
            .map(|v| (v, 0))
            .unwrap();
    assert_eq!(decoded.unwrap().into_vec(), [7]);

    // Borrowed slices and strings.
    let bytes = bincode::encode_to_vec(Some("hi"), config::standard().with_niches()).unwrap();
    assert_eq!(bytes, [2, b'h', b'i']);
    let (decoded, _): (Option<&str>, usize) =
        bincode::borrow_decode_from_slice(&bytes, config::standard().with_niches()).unwrap();
    assert_eq!(decoded, Some("hi"));
    let (decoded, _): (Option<&str>, usize) =
        bincode::borrow_decode_from_slice(&[254], config::standard().with_niches()).unwrap();
    assert_eq!(decoded, None);
}

#[test]
fn option_composites_use_first_field_niches() {
    check(Some((NonZeroU8::MIN, 7u8)), &[1, 7]);
    check(None::<(NonZeroU8, u8)>, &[0]);
    check(Some((true, 7u8, 8u8)), &[1, 7, 8]);
    check(Some([true, false]), &[1, 0]);
    check(None::<[bool; 2]>, &[2]);
    // An empty array writes nothing, so its option needs a tag.
    check(Some([true; 0]), &[1]);
    check(None::<[bool; 0]>, &[0]);

    check_varint(Some(Duration::new(5, 6)), &[5, 6]);
    check_varint(None::<Duration>, &[254]);
    check_varint(Some(1u32..2), &[1, 2]);
    check_varint(None::<Range<u32>>, &[253]);
    check_varint(Some(1u32..=2), &[1, 2]);
    check_varint(None::<RangeInclusive<u32>>, &[253]);
}

#[test]
fn option_enum_uses_variant_index_niches() {
    check_varint(Some(Ok::<u8, u8>(1)), &[0, 1]);
    check_varint(Some(Err::<u8, u8>(2)), &[1, 2]);
    check_varint(None::<Result<u8, u8>>, &[2]);
    check_varint(None::<Bound<u8>>, &[3]);
    check_varint(Some(Bound::Included(1u8)), &[1, 1]);
    check_varint(None::<IpAddr>, &[2]);
    check_varint(Some(IpAddr::V4(Ipv4Addr::LOCALHOST)), &[0, 127, 0, 0, 1]);
    check_varint(None::<SocketAddr>, &[2]);

    // Under fixed int encoding the index is four bytes. In little endian its low byte comes first.
    check_with_config(
        &Some(Ok::<u8, u8>(1)),
        &[0, 0, 0, 0, 1],
        config::legacy().with_niches(),
    );
    check_with_config(
        &None::<Result<u8, u8>>,
        &[2],
        config::legacy().with_niches(),
    );
    // In big endian its first byte is always zero.
    let big_endian = config::legacy().with_niches().with_big_endian();
    check_with_config(&Some(Err::<u8, u8>(1)), &[0, 0, 0, 1, 1], big_endian);
    check_with_config(&None::<Result<u8, u8>>, &[1], big_endian);
    check_with_config(&Some(None::<Result<u8, u8>>), &[1], big_endian);
    check_with_config(&None::<Option<Result<u8, u8>>>, &[2], big_endian);
}

#[test]
fn unclaimed_niches_are_rejected() {
    let err = bincode::decode_from_slice::<Option<bool>, _>(&[3], config::standard().with_niches())
        .unwrap_err();
    assert!(
        matches!(err, DecodeError::UnexpectedNiche { found: 3, .. }),
        "{err:?}"
    );

    let err = bincode::decode_from_slice::<Option<u8>, _>(&[2], config::standard().with_niches())
        .unwrap_err();
    assert!(
        matches!(err, DecodeError::UnexpectedVariant { found: 2, .. }),
        "{err:?}"
    );
}

/// Returns whether `bytes` decode as a `T` within `config`'s byte limit.
fn fits<T: Decode<()>, C: Config>(bytes: &[u8], config: C) -> bool {
    match bincode::decode_from_slice::<T, C>(bytes, config) {
        Ok(_) => true,
        Err(DecodeError::LimitExceeded) => false,
        Err(e) => panic!("{e:?}"),
    }
}

#[test]
fn niche_encoded_some_fits_the_same_limit_as_its_payload() {
    // A niche-encoded `Some(v)` is just `v`, so it must claim exactly as many bytes as `v`.
    macro_rules! assert_same_limit {
        ($ty:ty, $value:expr, $limit:literal) => {{
            let value: $ty = $value;
            let bytes = bincode::encode_to_vec(&value, config::standard().with_niches()).unwrap();
            let fits_at = config::standard().with_niches().with_limit::<$limit>();
            let too_small = config::standard()
                .with_niches()
                .with_limit::<{ $limit - 1 }>();
            assert!(fits::<$ty, _>(&bytes, fits_at), "{}", stringify!($ty));
            assert!(
                fits::<Option<$ty>, _>(&bytes, fits_at),
                "Option<{}>",
                stringify!($ty)
            );
            assert!(!fits::<$ty, _>(&bytes, too_small), "{}", stringify!($ty));
            assert!(
                !fits::<Option<$ty>, _>(&bytes, too_small),
                "Option<{}>",
                stringify!($ty)
            );
        }};
    }
    assert_same_limit!(u32, 7, 4);
    assert_same_limit!(NonZeroU64, NonZeroU64::MIN, 8);
    assert_same_limit!(String, String::from("hi"), 10);
    assert_same_limit!([u32; 2], [1, 2], 8);
    assert_same_limit!((u32, u8), (1, 2), 5);
    assert_same_limit!(Duration, Duration::new(1, 2), 12);
    assert_same_limit!(Result<u8, u8>, Ok(1), 5);
}

#[test]
fn niches_respect_the_byte_limit() {
    // Before niches, `Some(n)` needed two bytes.
    let one = config::standard().with_niches().with_limit::<1>();
    let (decoded, _): (Option<NonZeroU8>, usize) = bincode::decode_from_slice(&[7], one).unwrap();
    assert_eq!(decoded, NonZeroU8::new(7));

    let encoded = "é".as_bytes();
    let err = bincode::decode_from_slice::<Option<char>, _>(encoded, one).unwrap_err();
    assert!(matches!(err, DecodeError::LimitExceeded), "{err:?}");
    let two = config::standard().with_niches().with_limit::<2>();
    let (decoded, _): (Option<char>, usize) = bincode::decode_from_slice(encoded, two).unwrap();
    assert_eq!(decoded, Some('é'));
}

/// Checks that all three traits report the same niches for `T`, and that no niche byte begins a
/// valid encoding of `T`.
fn check_niches<T, C>(config: C, expected: Niches)
where
    T: Encode + Decode<()> + for<'de> BorrowDecode<'de, ()>,
    C: Config,
{
    let name = core::any::type_name::<T>();
    assert_eq!(<T as Encode>::ENCODE_NICHES.get::<C>(), expected, "{name}");
    assert_eq!(
        <T as Decode<()>>::DECODE_NICHES.get::<C>(),
        expected,
        "{name}"
    );
    assert_eq!(
        <T as BorrowDecode<'_, ()>>::BORROW_DECODE_NICHES.get::<C>(),
        expected,
        "{name}"
    );
    for byte in 0..=u8::MAX {
        if expected.contains(byte) {
            for filler in [0x00, 0xFF] {
                let mut input = [filler; 32];
                input[0] = byte;
                assert!(
                    bincode::decode_from_slice::<T, C>(&input, config).is_err(),
                    "{name} decoded from its niche {byte:#04x} followed by {filler:#04x}"
                );
            }
        }
    }
}

fn check_all_niches<C: Config>(config: C) {
    let varint = |niches| NicheTable::varint_only(niches).get::<C>();
    let index = |variant_count| NicheTable::variant_index(variant_count).get::<C>();

    check_niches::<u8, C>(config, Niches::NONE);
    check_niches::<bool, C>(config, Niches::range(2, 255));
    check_niches::<NonZeroU8, C>(config, Niches::range(0, 0));
    check_niches::<NonZeroI8, C>(config, Niches::range(0, 0));
    check_niches::<char, C>(config, Niches::range(0x80, 0xC1));

    check_niches::<Option<u8>, C>(config, Niches::range(2, 255));
    check_niches::<Option<NonZeroU8>, C>(config, Niches::NONE);
    check_niches::<Option<Option<NonZeroU8>>, C>(config, Niches::range(2, 255));
    check_niches::<Option<bool>, C>(config, Niches::range(3, 255));
    check_niches::<Option<Option<bool>>, C>(config, Niches::range(4, 255));
    check_niches::<Option<char>, C>(config, Niches::range(0x81, 0xC1));

    check_niches::<Box<bool>, C>(config, Niches::range(2, 255));
    check_niches::<Rc<char>, C>(config, Niches::range(0x80, 0xC1));
    check_niches::<Arc<NonZeroU8>, C>(config, Niches::range(0, 0));
    check_niches::<Wrapping<bool>, C>(config, Niches::range(2, 255));
    check_niches::<Reverse<bool>, C>(config, Niches::range(2, 255));
    check_niches::<Cell<bool>, C>(config, Niches::range(2, 255));
    check_niches::<RefCell<bool>, C>(config, Niches::range(2, 255));
    check_niches::<Mutex<bool>, C>(config, Niches::range(2, 255));

    check_niches::<u16, C>(config, varint(Niches::range(252, 255)));
    check_niches::<u32, C>(config, varint(Niches::range(253, 255)));
    check_niches::<u64, C>(config, varint(Niches::range(254, 255)));
    check_niches::<u128, C>(config, varint(Niches::range(255, 255)));
    check_niches::<usize, C>(config, varint(Niches::range(254, 255)));
    check_niches::<i16, C>(config, varint(Niches::range(252, 255)));
    check_niches::<i32, C>(config, varint(Niches::range(253, 255)));
    check_niches::<i64, C>(config, varint(Niches::range(254, 255)));
    check_niches::<i128, C>(config, varint(Niches::range(255, 255)));
    check_niches::<isize, C>(config, varint(Niches::range(254, 255)));
    check_niches::<NonZeroU32, C>(config, varint(Niches::range(253, 255)));
    check_niches::<NonZeroI32, C>(config, varint(Niches::range(253, 255)));
    check_niches::<AtomicBool, C>(config, Niches::range(2, 255));
    check_niches::<AtomicU32, C>(config, varint(Niches::range(253, 255)));

    let len = varint(Niches::range(254, 255));
    check_niches::<String, C>(config, len);
    check_niches::<Box<str>, C>(config, len);
    check_niches::<Rc<str>, C>(config, len);
    check_niches::<Arc<str>, C>(config, len);
    check_niches::<Vec<u32>, C>(config, len);
    check_niches::<Box<[u32]>, C>(config, len);
    check_niches::<Rc<[u32]>, C>(config, len);
    check_niches::<Arc<[u32]>, C>(config, len);
    check_niches::<VecDeque<u32>, C>(config, len);
    check_niches::<BinaryHeap<u32>, C>(config, len);
    check_niches::<BTreeMap<u32, u32>, C>(config, len);
    check_niches::<BTreeSet<u32>, C>(config, len);
    check_niches::<HashMap<u32, u32>, C>(config, len);
    check_niches::<HashSet<u32>, C>(config, len);
    check_niches::<CString, C>(config, len);
    check_niches::<PathBuf, C>(config, len);

    check_niches::<(bool, u32), C>(config, Niches::range(2, 255));
    check_niches::<(u32, bool), C>(config, varint(Niches::range(253, 255)));
    check_niches::<[bool; 3], C>(config, Niches::range(2, 255));
    check_niches::<[bool; 0], C>(config, Niches::NONE);
    check_niches::<Duration, C>(config, varint(Niches::range(254, 255)));
    check_niches::<Range<u32>, C>(config, varint(Niches::range(253, 255)));
    check_niches::<RangeInclusive<u32>, C>(config, varint(Niches::range(253, 255)));

    check_niches::<Result<u8, u8>, C>(config, index(2));
    check_niches::<Bound<u8>, C>(config, index(3));
    check_niches::<IpAddr, C>(config, index(2));
    check_niches::<SocketAddr, C>(config, index(2));
    check_niches::<Option<Result<u8, u8>>, C>(config, index(2).skip(1));
}

// Niche tables are constants, so they can be checked at compile time.
const _: () = assert!(
    <Option<Option<bool>> as Encode>::ENCODE_NICHES
        .get::<config::Configuration<
            config::LittleEndian,
            config::Varint,
            config::NoLimit,
            config::UseNiches,
        >>()
        .start()
        == 4
);

#[test]
fn niches_are_consistent() {
    check_all_niches(config::standard().with_niches());
    check_all_niches(config::standard().with_niches().with_big_endian());
    check_all_niches(config::legacy().with_niches());
    check_all_niches(config::legacy().with_niches().with_big_endian());
}

#[derive(Encode, Decode, PartialEq, Debug)]
struct Id {
    value: NonZeroU32,
    name: String,
}

#[derive(Encode, Decode, PartialEq, Debug)]
struct SkipsFirstField {
    #[bincode(skip)]
    cache: u32,
    flag: bool,
    count: u32,
}

#[derive(Encode, Decode, PartialEq, Debug)]
struct Tuple(NonZeroU8, u8);

#[derive(Encode, Decode, PartialEq, Debug)]
struct Wrapper<T> {
    inner: T,
}

#[derive(Encode, Decode, PartialEq, Debug)]
struct Outer {
    id: Id,
    extra: u8,
}

#[derive(Encode, Decode, PartialEq, Debug)]
enum Color {
    Red,
    Green,
    Blue(u8),
}

/// A first field with a lifetime gives the struct no niches.
#[derive(Encode, bincode::BorrowDecode, PartialEq, Debug)]
struct Borrowed<'a> {
    name: &'a str,
    count: u8,
}

#[derive(Encode, bincode::BorrowDecode, PartialEq, Debug)]
struct BorrowedSecond<'a> {
    count: NonZeroU8,
    name: &'a str,
}

/// With `#[bincode(no_niche)]`, a derived type declares no niches.
#[derive(Encode, Decode, PartialEq, Debug)]
#[bincode(no_niche)]
struct Plain {
    flag: bool,
}

#[test]
fn derived_structs_take_their_first_field_niches() {
    let id = Id {
        value: NonZeroU32::new(5).unwrap(),
        name: String::from("a"),
    };
    check_varint(Some(id), &[5, 1, b'a']);
    check_varint(None::<Id>, &[253]);

    let skips = SkipsFirstField {
        cache: 0,
        flag: true,
        count: 7,
    };
    check_varint(Some(skips), &[1, 7]);
    check_varint(None::<SkipsFirstField>, &[2]);

    check(Some(Tuple(NonZeroU8::MIN, 7)), &[1, 7]);
    check(None::<Tuple>, &[0]);

    check(Some(Wrapper { inner: true }), &[1]);
    check(None::<Wrapper<bool>>, &[2]);
    check(None::<Wrapper<Wrapper<char>>>, &[0x80]);

    let outer = Outer {
        id: Id {
            value: NonZeroU32::new(5).unwrap(),
            name: String::new(),
        },
        extra: 9,
    };
    check_varint(Some(outer), &[5, 0, 9]);
    check_varint(None::<Outer>, &[253]);

    check(Some(Plain { flag: true }), &[1, 1]);
    check(None::<Plain>, &[0]);
}

#[test]
fn derived_enums_take_their_unused_variant_indices() {
    check_varint(Some(Color::Red), &[0]);
    check_varint(Some(Color::Blue(9)), &[2, 9]);
    check_varint(None::<Color>, &[3]);
    check_varint(Some(None::<Color>), &[3]);
    check_varint(None::<Option<Color>>, &[4]);
    check_with_config(
        &Some(Color::Green),
        &[1, 0, 0, 0],
        config::legacy().with_niches(),
    );
    check_with_config(&None::<Color>, &[3], config::legacy().with_niches());
    check_with_config(
        &None::<Color>,
        &[1],
        config::legacy().with_niches().with_big_endian(),
    );
}

#[test]
fn derived_borrow_decode() {
    let config = config::standard().with_niches();

    let value = Some(BorrowedSecond {
        count: NonZeroU8::new(3).unwrap(),
        name: "x",
    });
    let bytes = bincode::encode_to_vec(&value, config).unwrap();
    assert_eq!(bytes, [3, 1, b'x']);
    let (decoded, _): (Option<BorrowedSecond>, usize) =
        bincode::borrow_decode_from_slice(&bytes, config).unwrap();
    assert_eq!(decoded, value);
    let (decoded, _): (Option<BorrowedSecond>, usize) =
        bincode::borrow_decode_from_slice(&[0], config).unwrap();
    assert_eq!(decoded, None);

    let value = Some(Borrowed {
        name: "x",
        count: 3,
    });
    let bytes = bincode::encode_to_vec(&value, config).unwrap();
    assert_eq!(bytes, [1, 1, b'x', 3]);
    let (decoded, _): (Option<Borrowed>, usize) =
        bincode::borrow_decode_from_slice(&bytes, config).unwrap();
    assert_eq!(decoded, value);
    assert_eq!(
        bincode::encode_to_vec(None::<Borrowed>, config).unwrap(),
        [0]
    );

    fn check_borrowed_niches<C: Config>() {
        assert_eq!(<Borrowed as Encode>::ENCODE_NICHES.get::<C>(), Niches::NONE);
        assert_eq!(
            <Borrowed as BorrowDecode<'_, ()>>::BORROW_DECODE_NICHES.get::<C>(),
            Niches::NONE
        );
        let expected = Niches::range(0, 0);
        assert_eq!(
            <BorrowedSecond as Encode>::ENCODE_NICHES.get::<C>(),
            expected
        );
        assert_eq!(
            <BorrowedSecond as BorrowDecode<'_, ()>>::BORROW_DECODE_NICHES.get::<C>(),
            expected
        );
    }
    check_borrowed_niches::<
        config::Configuration<
            config::LittleEndian,
            config::Varint,
            config::NoLimit,
            config::UseNiches,
        >,
    >();
}

fn check_derived_niches<C: Config>(config: C) {
    let varint = |niches| NicheTable::varint_only(niches).get::<C>();
    check_niches::<Id, C>(config, varint(Niches::range(253, 255)));
    check_niches::<SkipsFirstField, C>(config, Niches::range(2, 255));
    check_niches::<Tuple, C>(config, Niches::range(0, 0));
    check_niches::<Wrapper<bool>, C>(config, Niches::range(2, 255));
    check_niches::<Wrapper<u8>, C>(config, Niches::NONE);
    check_niches::<Outer, C>(config, varint(Niches::range(253, 255)));
    check_niches::<Color, C>(config, NicheTable::variant_index(3).get::<C>());
    check_niches::<Plain, C>(config, Niches::NONE);
}

#[test]
fn derived_niches_are_consistent() {
    check_derived_niches(config::standard().with_niches());
    check_derived_niches(config::standard().with_niches().with_big_endian());
    check_derived_niches(config::legacy().with_niches());
    check_derived_niches(config::legacy().with_niches().with_big_endian());
}

/// A struct whose first field contains itself must opt out, or its niches would depend on
/// themselves.
#[derive(Encode, Decode, PartialEq, Debug)]
#[bincode(no_niche)]
struct Node {
    next: Option<Box<Node>>,
    value: u8,
}

/// The niches of a `Vec` don't depend on its elements, so this needs no opt-out.
#[derive(Encode, Decode, PartialEq, Debug)]
struct Tree {
    children: Vec<Tree>,
    value: u8,
}

#[test]
fn recursive_types() {
    let node = Node {
        next: Some(Box::new(Node {
            next: None,
            value: 2,
        })),
        value: 1,
    };
    // The inner option's payload is `Box<Node>`, which has no niches, so it keeps its tag.
    check(Some(node), &[1, 1, 0, 2, 1]);

    let tree = Tree {
        children: vec![Tree {
            children: Vec::new(),
            value: 2,
        }],
        value: 1,
    };
    check_varint(Some(tree), &[1, 0, 2, 1]);
    check_varint(None::<Tree>, &[254]);
}

#[test]
fn default_configs_do_not_use_niches() {
    // Without `with_niches`, every option writes a tag, exactly as before niches existed.
    fn check_varint_config<C: Config>(config: C) {
        assert_eq!(
            bincode::encode_to_vec(NonZeroU8::new(5), config).unwrap(),
            [1, 5]
        );
        assert_eq!(
            bincode::encode_to_vec(None::<NonZeroU8>, config).unwrap(),
            [0]
        );
        assert_eq!(bincode::encode_to_vec(None::<bool>, config).unwrap(), [0]);
        assert_eq!(bincode::encode_to_vec(Some(7u32), config).unwrap(), [1, 7]);
        assert_eq!(
            bincode::encode_to_vec(Some("hi"), config).unwrap(),
            [1, 2, b'h', b'i']
        );
        assert_eq!(bincode::encode_to_vec(None::<Color>, config).unwrap(), [0]);
        assert_eq!(
            bincode::encode_to_vec(Some(Color::Red), config).unwrap(),
            [1, 0]
        );
    }
    check_varint_config(config::standard());
    check_varint_config(config::standard().with_big_endian());
    let legacy = config::legacy();
    assert_eq!(
        bincode::encode_to_vec(NonZeroU8::new(5), legacy).unwrap(),
        [1, 5]
    );
    assert_eq!(
        bincode::encode_to_vec(Some(Ok::<u8, u8>(1)), legacy).unwrap(),
        [1, 0, 0, 0, 0, 1]
    );

    fn assert_no_niches<T: Encode + Decode<()> + for<'de> BorrowDecode<'de, ()>, C: Config>() {
        let name = core::any::type_name::<T>();
        assert_eq!(
            <T as Encode>::ENCODE_NICHES.get::<C>(),
            Niches::NONE,
            "{name}"
        );
        assert_eq!(
            <T as Decode<()>>::DECODE_NICHES.get::<C>(),
            Niches::NONE,
            "{name}"
        );
        assert_eq!(
            <T as BorrowDecode<'_, ()>>::BORROW_DECODE_NICHES.get::<C>(),
            Niches::NONE,
            "{name}"
        );
    }
    fn check_config<C: Config>(_: C) {
        assert_no_niches::<bool, C>();
        assert_no_niches::<NonZeroU8, C>();
        assert_no_niches::<char, C>();
        assert_no_niches::<u32, C>();
        assert_no_niches::<String, C>();
        assert_no_niches::<Option<u8>, C>();
        assert_no_niches::<Result<u8, u8>, C>();
        assert_no_niches::<Id, C>();
        assert_no_niches::<Color, C>();
    }
    check_config(config::standard());
    check_config(config::legacy());
}
