use crate::{
    config::Endianness,
    de::read::Reader,
    error::{DecodeError, IntegerType},
};

/// Reverses the zigzag encoding of a signed integer.
macro_rules! zigzag_decode {
    ($n:expr) => {{
        let n = $n;
        if n % 2 == 0 {
            // positive number
            (n / 2) as _
        } else {
            // negative number
            // !m * 2 + 1 = n
            // !m * 2 = n - 1
            // !m = (n - 1) / 2
            // m = !((n - 1) / 2)
            // since we have n is odd, we have floor(n / 2) = floor((n - 1) / 2)
            !(n / 2) as _
        }
    }};
}

pub fn varint_decode_i16<R: Reader>(read: &mut R, endian: Endianness) -> Result<i16, DecodeError> {
    let n = super::varint_decode_u16(read, endian)
        .map_err(DecodeError::change_integer_type_to_signed)?;
    Ok(zigzag_decode!(n))
}

/// Decodes a varint `i16` whose first byte, `first`, has already been read from `read`.
#[inline]
pub fn varint_decode_i16_with_first_byte<R: Reader>(
    first: u8,
    read: &mut R,
    endian: Endianness,
) -> Result<i16, DecodeError> {
    let n = super::varint_decode_u16_with_first_byte(first, read, endian)
        .map_err(DecodeError::change_integer_type_to_signed)?;
    Ok(zigzag_decode!(n))
}

pub fn varint_decode_i32<R: Reader>(read: &mut R, endian: Endianness) -> Result<i32, DecodeError> {
    let n = super::varint_decode_u32(read, endian)
        .map_err(DecodeError::change_integer_type_to_signed)?;
    Ok(zigzag_decode!(n))
}

/// Decodes a varint `i32` whose first byte, `first`, has already been read from `read`.
#[inline]
pub fn varint_decode_i32_with_first_byte<R: Reader>(
    first: u8,
    read: &mut R,
    endian: Endianness,
) -> Result<i32, DecodeError> {
    let n = super::varint_decode_u32_with_first_byte(first, read, endian)
        .map_err(DecodeError::change_integer_type_to_signed)?;
    Ok(zigzag_decode!(n))
}

pub fn varint_decode_i64<R: Reader>(read: &mut R, endian: Endianness) -> Result<i64, DecodeError> {
    let n = super::varint_decode_u64(read, endian)
        .map_err(DecodeError::change_integer_type_to_signed)?;
    Ok(zigzag_decode!(n))
}

/// Decodes a varint `i64` whose first byte, `first`, has already been read from `read`.
#[inline]
pub fn varint_decode_i64_with_first_byte<R: Reader>(
    first: u8,
    read: &mut R,
    endian: Endianness,
) -> Result<i64, DecodeError> {
    let n = super::varint_decode_u64_with_first_byte(first, read, endian)
        .map_err(DecodeError::change_integer_type_to_signed)?;
    Ok(zigzag_decode!(n))
}

pub fn varint_decode_i128<R: Reader>(
    read: &mut R,
    endian: Endianness,
) -> Result<i128, DecodeError> {
    let n = super::varint_decode_u128(read, endian)
        .map_err(DecodeError::change_integer_type_to_signed)?;
    Ok(zigzag_decode!(n))
}

/// Decodes a varint `i128` whose first byte, `first`, has already been read from `read`.
#[inline]
pub fn varint_decode_i128_with_first_byte<R: Reader>(
    first: u8,
    read: &mut R,
    endian: Endianness,
) -> Result<i128, DecodeError> {
    let n = super::varint_decode_u128_with_first_byte(first, read, endian)
        .map_err(DecodeError::change_integer_type_to_signed)?;
    Ok(zigzag_decode!(n))
}

pub fn varint_decode_isize<R: Reader>(
    read: &mut R,
    endian: Endianness,
) -> Result<isize, DecodeError> {
    isize_from_i64(varint_decode_i64(read, endian))
}

/// Decodes a varint `isize` whose first byte, `first`, has already been read from `read`.
#[inline]
pub fn varint_decode_isize_with_first_byte<R: Reader>(
    first: u8,
    read: &mut R,
    endian: Endianness,
) -> Result<isize, DecodeError> {
    isize_from_i64(varint_decode_i64_with_first_byte(first, read, endian))
}

/// `isize` is encoded as an `i64`; this reports errors in terms of `isize`.
#[inline]
fn isize_from_i64(result: Result<i64, DecodeError>) -> Result<isize, DecodeError> {
    match result {
        Ok(val) => Ok(val as isize),
        Err(DecodeError::InvalidIntegerType { found, .. }) => {
            Err(DecodeError::InvalidIntegerType {
                expected: IntegerType::Isize,
                found: found.into_signed(),
            })
        }
        Err(e) => Err(e),
    }
}
