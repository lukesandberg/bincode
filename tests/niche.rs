#![cfg(feature = "std")]

use bincode::{
    config::{self, Config},
    error::DecodeError,
    niche::Niches,
    BorrowDecode, Decode, Encode,
};
use core::{
    cell::{Cell, RefCell},
    cmp::Reverse,
    fmt::Debug,
    num::{NonZeroI8, NonZeroU8, Wrapping},
};
use std::{rc::Rc, sync::Arc, sync::Mutex};

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
    check_with_config(&value, expected, config::standard());
    check_with_config(&value, expected, config::standard().with_big_endian());
    check_with_config(&value, expected, config::legacy());
    check_with_config(&value, expected, config::legacy().with_big_endian());
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
        bincode::encode_to_vec(None::<&bool>, config::standard()).unwrap(),
        [2]
    );

    let bytes = bincode::encode_to_vec(None::<Mutex<bool>>, config::standard()).unwrap();
    assert_eq!(bytes, [2]);
    let (decoded, _): (Option<Mutex<bool>>, usize) =
        bincode::decode_from_slice(&[1], config::standard()).unwrap();
    assert!(*decoded.unwrap().lock().unwrap());
}

#[test]
fn unclaimed_niches_are_rejected() {
    let err = bincode::decode_from_slice::<Option<bool>, _>(&[3], config::standard()).unwrap_err();
    assert!(
        matches!(err, DecodeError::UnexpectedNiche { found: 3, .. }),
        "{err:?}"
    );

    let err = bincode::decode_from_slice::<Option<u8>, _>(&[2], config::standard()).unwrap_err();
    assert!(
        matches!(err, DecodeError::UnexpectedVariant { found: 2, .. }),
        "{err:?}"
    );
}

#[test]
fn niches_respect_the_byte_limit() {
    // Before niches, `Some(n)` needed two bytes.
    let one = config::standard().with_limit::<1>();
    let (decoded, _): (Option<NonZeroU8>, usize) = bincode::decode_from_slice(&[7], one).unwrap();
    assert_eq!(decoded, NonZeroU8::new(7));

    let encoded = "é".as_bytes();
    let err = bincode::decode_from_slice::<Option<char>, _>(encoded, one).unwrap_err();
    assert!(matches!(err, DecodeError::LimitExceeded), "{err:?}");
    let two = config::standard().with_limit::<2>();
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
            let mut input = [0u8; 32];
            input[0] = byte;
            assert!(
                bincode::decode_from_slice::<T, C>(&input, config).is_err(),
                "{name} decoded from its niche {byte:#04x}"
            );
        }
    }
}

fn check_all_niches<C: Config>(config: C) {
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
}

// Niche tables are constants, so they can be checked at compile time.
const _: () = assert!(
    <Option<Option<bool>> as Encode>::ENCODE_NICHES
        .get::<config::Configuration>()
        .start()
        == 4
);

#[test]
fn niches_are_consistent() {
    check_all_niches(config::standard());
    check_all_niches(config::standard().with_big_endian());
    check_all_niches(config::legacy());
    check_all_niches(config::legacy().with_big_endian());
}
