use super::{
    read::{BorrowReader, Reader},
    BorrowDecode, BorrowDecoder, Decode, Decoder, OptionFirstByte,
};
use crate::{
    config::{Endianness, IntEncoding, InternalEndianConfig, InternalIntEncodingConfig},
    error::{DecodeError, IntegerType},
    impl_borrow_decode,
    niche::{NicheTable, Niches, SLICE_LEN_NICHES},
};
use core::{
    cell::{Cell, RefCell},
    cmp::Reverse,
    num::{
        NonZeroI128, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroIsize, NonZeroU128,
        NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize, Wrapping,
    },
    ops::{Bound, Range, RangeInclusive},
    time::Duration,
};

/// Adds the niche items of [Decode] to the impl of an integer: under varint encoding it never
/// begins with the tags of wider integers.
macro_rules! impl_varint_niches {
    ($ty:ty, $niches:expr, $varint_decode_with_first_byte:path) => {
        const DECODE_NICHES: NicheTable = NicheTable::varint_only($niches);

        fn decode_with_first_byte<D: Decoder<Context = Context>>(
            first: u8,
            decoder: &mut D,
        ) -> Result<Self, DecodeError> {
            // `decode` claims the full width up front, and the first byte is already claimed.
            decoder.claim_bytes_read(core::mem::size_of::<$ty>() - 1)?;
            match D::C::INT_ENCODING {
                IntEncoding::Variable => {
                    $varint_decode_with_first_byte(first, decoder.reader(), D::C::ENDIAN)
                }
                IntEncoding::Fixed => {
                    let bytes = read_fixint_rest(first, decoder)?;
                    Ok(match D::C::ENDIAN {
                        Endianness::Little => <$ty>::from_le_bytes(bytes),
                        Endianness::Big => <$ty>::from_be_bytes(bytes),
                    })
                }
            }
        }
    };
}

/// Adds the niche items of [Decode] to the impl of a `NonZero` integer, which has the same niches
/// as the integer it wraps.
macro_rules! impl_non_zero_niches {
    ($ty:ty, $int:ty, $integer_type:expr) => {
        const DECODE_NICHES: NicheTable = <$int as Decode<Context>>::DECODE_NICHES;

        fn decode_with_first_byte<D: Decoder<Context = Context>>(
            first: u8,
            decoder: &mut D,
        ) -> Result<Self, DecodeError> {
            <$ty>::new(<$int>::decode_with_first_byte(first, decoder)?).ok_or(
                DecodeError::NonZeroTypeIsZero {
                    non_zero_type: $integer_type,
                },
            )
        }
    };
}

/// Reads the rest of a fixed-width integer whose first byte has already been read.
#[inline]
fn read_fixint_rest<D: Decoder, const N: usize>(
    first: u8,
    decoder: &mut D,
) -> Result<[u8; N], DecodeError> {
    let mut bytes = [0u8; N];
    bytes[0] = first;
    decoder.reader().read(&mut bytes[1..])?;
    Ok(bytes)
}

impl<Context> Decode<Context> for bool {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let first = u8::decode(decoder)?;
        Self::decode_with_first_byte(first, decoder)
    }

    const DECODE_NICHES: NicheTable = NicheTable::uniform(Niches::range(2, u8::MAX));

    #[inline]
    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        _: &mut D,
    ) -> Result<Self, DecodeError> {
        match first {
            0 => Ok(false),
            1 => Ok(true),
            x => Err(DecodeError::InvalidBooleanValue(x)),
        }
    }
}
impl_borrow_decode!(bool);

impl<Context> Decode<Context> for u8 {
    #[inline]
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        decoder.claim_bytes_read(1)?;
        if let Some(buf) = decoder.reader().peek_read(1) {
            let byte = buf[0];
            decoder.reader().consume(1);
            Ok(byte)
        } else {
            let mut bytes = [0u8; 1];
            decoder.reader().read(&mut bytes)?;
            Ok(bytes[0])
        }
    }
}
impl_borrow_decode!(u8);

impl<Context> Decode<Context> for NonZeroU8 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let first = u8::decode(decoder)?;
        Self::decode_with_first_byte(first, decoder)
    }

    const DECODE_NICHES: NicheTable = NicheTable::uniform(Niches::range(0, 0));

    #[inline]
    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        _: &mut D,
    ) -> Result<Self, DecodeError> {
        NonZeroU8::new(first).ok_or(DecodeError::NonZeroTypeIsZero {
            non_zero_type: IntegerType::U8,
        })
    }
}
impl_borrow_decode!(NonZeroU8);

impl<Context> Decode<Context> for u16 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        decoder.claim_bytes_read(2)?;
        match D::C::INT_ENCODING {
            IntEncoding::Variable => {
                crate::varint::varint_decode_u16(decoder.reader(), D::C::ENDIAN)
            }
            IntEncoding::Fixed => {
                let mut bytes = [0u8; 2];
                decoder.reader().read(&mut bytes)?;
                Ok(match D::C::ENDIAN {
                    Endianness::Little => u16::from_le_bytes(bytes),
                    Endianness::Big => u16::from_be_bytes(bytes),
                })
            }
        }
    }

    impl_varint_niches!(
        u16,
        crate::varint::U16_NICHES,
        crate::varint::varint_decode_u16_with_first_byte
    );
}
impl_borrow_decode!(u16);

impl<Context> Decode<Context> for NonZeroU16 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        NonZeroU16::new(u16::decode(decoder)?).ok_or(DecodeError::NonZeroTypeIsZero {
            non_zero_type: IntegerType::U16,
        })
    }

    impl_non_zero_niches!(NonZeroU16, u16, IntegerType::U16);
}
impl_borrow_decode!(NonZeroU16);

impl<Context> Decode<Context> for u32 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        decoder.claim_bytes_read(4)?;
        match D::C::INT_ENCODING {
            IntEncoding::Variable => {
                crate::varint::varint_decode_u32(decoder.reader(), D::C::ENDIAN)
            }
            IntEncoding::Fixed => {
                let mut bytes = [0u8; 4];
                decoder.reader().read(&mut bytes)?;
                Ok(match D::C::ENDIAN {
                    Endianness::Little => u32::from_le_bytes(bytes),
                    Endianness::Big => u32::from_be_bytes(bytes),
                })
            }
        }
    }

    impl_varint_niches!(
        u32,
        crate::varint::U32_NICHES,
        crate::varint::varint_decode_u32_with_first_byte
    );
}
impl_borrow_decode!(u32);

impl<Context> Decode<Context> for NonZeroU32 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        NonZeroU32::new(u32::decode(decoder)?).ok_or(DecodeError::NonZeroTypeIsZero {
            non_zero_type: IntegerType::U32,
        })
    }

    impl_non_zero_niches!(NonZeroU32, u32, IntegerType::U32);
}
impl_borrow_decode!(NonZeroU32);

impl<Context> Decode<Context> for u64 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        decoder.claim_bytes_read(8)?;
        match D::C::INT_ENCODING {
            IntEncoding::Variable => {
                crate::varint::varint_decode_u64(decoder.reader(), D::C::ENDIAN)
            }
            IntEncoding::Fixed => {
                let mut bytes = [0u8; 8];
                decoder.reader().read(&mut bytes)?;
                Ok(match D::C::ENDIAN {
                    Endianness::Little => u64::from_le_bytes(bytes),
                    Endianness::Big => u64::from_be_bytes(bytes),
                })
            }
        }
    }

    impl_varint_niches!(
        u64,
        crate::varint::U64_NICHES,
        crate::varint::varint_decode_u64_with_first_byte
    );
}
impl_borrow_decode!(u64);

impl<Context> Decode<Context> for NonZeroU64 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        NonZeroU64::new(u64::decode(decoder)?).ok_or(DecodeError::NonZeroTypeIsZero {
            non_zero_type: IntegerType::U64,
        })
    }

    impl_non_zero_niches!(NonZeroU64, u64, IntegerType::U64);
}
impl_borrow_decode!(NonZeroU64);

impl<Context> Decode<Context> for u128 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        decoder.claim_bytes_read(16)?;
        match D::C::INT_ENCODING {
            IntEncoding::Variable => {
                crate::varint::varint_decode_u128(decoder.reader(), D::C::ENDIAN)
            }
            IntEncoding::Fixed => {
                let mut bytes = [0u8; 16];
                decoder.reader().read(&mut bytes)?;
                Ok(match D::C::ENDIAN {
                    Endianness::Little => u128::from_le_bytes(bytes),
                    Endianness::Big => u128::from_be_bytes(bytes),
                })
            }
        }
    }

    impl_varint_niches!(
        u128,
        crate::varint::U128_NICHES,
        crate::varint::varint_decode_u128_with_first_byte
    );
}
impl_borrow_decode!(u128);

impl<Context> Decode<Context> for NonZeroU128 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        NonZeroU128::new(u128::decode(decoder)?).ok_or(DecodeError::NonZeroTypeIsZero {
            non_zero_type: IntegerType::U128,
        })
    }

    impl_non_zero_niches!(NonZeroU128, u128, IntegerType::U128);
}
impl_borrow_decode!(NonZeroU128);

impl<Context> Decode<Context> for usize {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        decoder.claim_bytes_read(8)?;
        match D::C::INT_ENCODING {
            IntEncoding::Variable => {
                crate::varint::varint_decode_usize(decoder.reader(), D::C::ENDIAN)
            }
            IntEncoding::Fixed => {
                let mut bytes = [0u8; 8];
                decoder.reader().read(&mut bytes)?;

                let value = match D::C::ENDIAN {
                    Endianness::Little => u64::from_le_bytes(bytes),
                    Endianness::Big => u64::from_be_bytes(bytes),
                };

                value
                    .try_into()
                    .map_err(|_| DecodeError::OutsideUsizeRange(value))
            }
        }
    }

    const DECODE_NICHES: NicheTable = NicheTable::varint_only(crate::varint::U64_NICHES);

    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        // `decode` claims 8 bytes up front, and the first byte is already claimed.
        decoder.claim_bytes_read(7)?;
        match D::C::INT_ENCODING {
            IntEncoding::Variable => crate::varint::varint_decode_usize_with_first_byte(
                first,
                decoder.reader(),
                D::C::ENDIAN,
            ),
            IntEncoding::Fixed => {
                let bytes = read_fixint_rest(first, decoder)?;
                let value = match D::C::ENDIAN {
                    Endianness::Little => u64::from_le_bytes(bytes),
                    Endianness::Big => u64::from_be_bytes(bytes),
                };
                value
                    .try_into()
                    .map_err(|_| DecodeError::OutsideUsizeRange(value))
            }
        }
    }
}
impl_borrow_decode!(usize);

impl<Context> Decode<Context> for NonZeroUsize {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        NonZeroUsize::new(usize::decode(decoder)?).ok_or(DecodeError::NonZeroTypeIsZero {
            non_zero_type: IntegerType::Usize,
        })
    }

    impl_non_zero_niches!(NonZeroUsize, usize, IntegerType::Usize);
}
impl_borrow_decode!(NonZeroUsize);

impl<Context> Decode<Context> for i8 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        decoder.claim_bytes_read(1)?;
        let mut bytes = [0u8; 1];
        decoder.reader().read(&mut bytes)?;
        Ok(bytes[0] as i8)
    }
}
impl_borrow_decode!(i8);

impl<Context> Decode<Context> for NonZeroI8 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let first = u8::decode(decoder)?;
        Self::decode_with_first_byte(first, decoder)
    }

    const DECODE_NICHES: NicheTable = NicheTable::uniform(Niches::range(0, 0));

    #[inline]
    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        _: &mut D,
    ) -> Result<Self, DecodeError> {
        NonZeroI8::new(first as i8).ok_or(DecodeError::NonZeroTypeIsZero {
            non_zero_type: IntegerType::I8,
        })
    }
}
impl_borrow_decode!(NonZeroI8);

impl<Context> Decode<Context> for i16 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        decoder.claim_bytes_read(2)?;
        match D::C::INT_ENCODING {
            IntEncoding::Variable => {
                crate::varint::varint_decode_i16(decoder.reader(), D::C::ENDIAN)
            }
            IntEncoding::Fixed => {
                let mut bytes = [0u8; 2];
                decoder.reader().read(&mut bytes)?;
                Ok(match D::C::ENDIAN {
                    Endianness::Little => i16::from_le_bytes(bytes),
                    Endianness::Big => i16::from_be_bytes(bytes),
                })
            }
        }
    }

    impl_varint_niches!(
        i16,
        crate::varint::U16_NICHES,
        crate::varint::varint_decode_i16_with_first_byte
    );
}
impl_borrow_decode!(i16);

impl<Context> Decode<Context> for NonZeroI16 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        NonZeroI16::new(i16::decode(decoder)?).ok_or(DecodeError::NonZeroTypeIsZero {
            non_zero_type: IntegerType::I16,
        })
    }

    impl_non_zero_niches!(NonZeroI16, i16, IntegerType::I16);
}
impl_borrow_decode!(NonZeroI16);

impl<Context> Decode<Context> for i32 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        decoder.claim_bytes_read(4)?;
        match D::C::INT_ENCODING {
            IntEncoding::Variable => {
                crate::varint::varint_decode_i32(decoder.reader(), D::C::ENDIAN)
            }
            IntEncoding::Fixed => {
                let mut bytes = [0u8; 4];
                decoder.reader().read(&mut bytes)?;
                Ok(match D::C::ENDIAN {
                    Endianness::Little => i32::from_le_bytes(bytes),
                    Endianness::Big => i32::from_be_bytes(bytes),
                })
            }
        }
    }

    impl_varint_niches!(
        i32,
        crate::varint::U32_NICHES,
        crate::varint::varint_decode_i32_with_first_byte
    );
}
impl_borrow_decode!(i32);

impl<Context> Decode<Context> for NonZeroI32 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        NonZeroI32::new(i32::decode(decoder)?).ok_or(DecodeError::NonZeroTypeIsZero {
            non_zero_type: IntegerType::I32,
        })
    }

    impl_non_zero_niches!(NonZeroI32, i32, IntegerType::I32);
}
impl_borrow_decode!(NonZeroI32);

impl<Context> Decode<Context> for i64 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        decoder.claim_bytes_read(8)?;
        match D::C::INT_ENCODING {
            IntEncoding::Variable => {
                crate::varint::varint_decode_i64(decoder.reader(), D::C::ENDIAN)
            }
            IntEncoding::Fixed => {
                let mut bytes = [0u8; 8];
                decoder.reader().read(&mut bytes)?;
                Ok(match D::C::ENDIAN {
                    Endianness::Little => i64::from_le_bytes(bytes),
                    Endianness::Big => i64::from_be_bytes(bytes),
                })
            }
        }
    }

    impl_varint_niches!(
        i64,
        crate::varint::U64_NICHES,
        crate::varint::varint_decode_i64_with_first_byte
    );
}
impl_borrow_decode!(i64);

impl<Context> Decode<Context> for NonZeroI64 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        NonZeroI64::new(i64::decode(decoder)?).ok_or(DecodeError::NonZeroTypeIsZero {
            non_zero_type: IntegerType::I64,
        })
    }

    impl_non_zero_niches!(NonZeroI64, i64, IntegerType::I64);
}
impl_borrow_decode!(NonZeroI64);

impl<Context> Decode<Context> for i128 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        decoder.claim_bytes_read(16)?;
        match D::C::INT_ENCODING {
            IntEncoding::Variable => {
                crate::varint::varint_decode_i128(decoder.reader(), D::C::ENDIAN)
            }
            IntEncoding::Fixed => {
                let mut bytes = [0u8; 16];
                decoder.reader().read(&mut bytes)?;
                Ok(match D::C::ENDIAN {
                    Endianness::Little => i128::from_le_bytes(bytes),
                    Endianness::Big => i128::from_be_bytes(bytes),
                })
            }
        }
    }

    impl_varint_niches!(
        i128,
        crate::varint::U128_NICHES,
        crate::varint::varint_decode_i128_with_first_byte
    );
}
impl_borrow_decode!(i128);

impl<Context> Decode<Context> for NonZeroI128 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        NonZeroI128::new(i128::decode(decoder)?).ok_or(DecodeError::NonZeroTypeIsZero {
            non_zero_type: IntegerType::I128,
        })
    }

    impl_non_zero_niches!(NonZeroI128, i128, IntegerType::I128);
}
impl_borrow_decode!(NonZeroI128);

impl<Context> Decode<Context> for isize {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        decoder.claim_bytes_read(8)?;
        match D::C::INT_ENCODING {
            IntEncoding::Variable => {
                crate::varint::varint_decode_isize(decoder.reader(), D::C::ENDIAN)
            }
            IntEncoding::Fixed => {
                let mut bytes = [0u8; 8];
                decoder.reader().read(&mut bytes)?;
                Ok(match D::C::ENDIAN {
                    Endianness::Little => i64::from_le_bytes(bytes),
                    Endianness::Big => i64::from_be_bytes(bytes),
                } as isize)
            }
        }
    }

    const DECODE_NICHES: NicheTable = NicheTable::varint_only(crate::varint::U64_NICHES);

    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        // `decode` claims 8 bytes up front, and the first byte is already claimed.
        decoder.claim_bytes_read(7)?;
        match D::C::INT_ENCODING {
            IntEncoding::Variable => crate::varint::varint_decode_isize_with_first_byte(
                first,
                decoder.reader(),
                D::C::ENDIAN,
            ),
            IntEncoding::Fixed => {
                let bytes = read_fixint_rest(first, decoder)?;
                Ok(match D::C::ENDIAN {
                    Endianness::Little => i64::from_le_bytes(bytes),
                    Endianness::Big => i64::from_be_bytes(bytes),
                } as isize)
            }
        }
    }
}
impl_borrow_decode!(isize);

impl<Context> Decode<Context> for NonZeroIsize {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        NonZeroIsize::new(isize::decode(decoder)?).ok_or(DecodeError::NonZeroTypeIsZero {
            non_zero_type: IntegerType::Isize,
        })
    }

    impl_non_zero_niches!(NonZeroIsize, isize, IntegerType::Isize);
}
impl_borrow_decode!(NonZeroIsize);

impl<Context> Decode<Context> for f32 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        decoder.claim_bytes_read(4)?;
        let mut bytes = [0u8; 4];
        decoder.reader().read(&mut bytes)?;
        Ok(match D::C::ENDIAN {
            Endianness::Little => f32::from_le_bytes(bytes),
            Endianness::Big => f32::from_be_bytes(bytes),
        })
    }
}
impl_borrow_decode!(f32);

impl<Context> Decode<Context> for f64 {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        decoder.claim_bytes_read(8)?;
        let mut bytes = [0u8; 8];
        decoder.reader().read(&mut bytes)?;
        Ok(match D::C::ENDIAN {
            Endianness::Little => f64::from_le_bytes(bytes),
            Endianness::Big => f64::from_be_bytes(bytes),
        })
    }
}
impl_borrow_decode!(f64);

impl<Context, T: Decode<Context>> Decode<Context> for Wrapping<T> {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        Ok(Wrapping(T::decode(decoder)?))
    }

    const DECODE_NICHES: NicheTable = T::DECODE_NICHES;

    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        Ok(Wrapping(T::decode_with_first_byte(first, decoder)?))
    }
}
impl<'de, Context, T: BorrowDecode<'de, Context>> BorrowDecode<'de, Context> for Wrapping<T> {
    fn borrow_decode<D: BorrowDecoder<'de, Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        Ok(Wrapping(T::borrow_decode(decoder)?))
    }

    const BORROW_DECODE_NICHES: NicheTable = T::BORROW_DECODE_NICHES;

    fn borrow_decode_with_first_byte<D: BorrowDecoder<'de, Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        Ok(Wrapping(T::borrow_decode_with_first_byte(first, decoder)?))
    }
}

impl<Context, T: Decode<Context>> Decode<Context> for Reverse<T> {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        Ok(Reverse(T::decode(decoder)?))
    }

    const DECODE_NICHES: NicheTable = T::DECODE_NICHES;

    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        Ok(Reverse(T::decode_with_first_byte(first, decoder)?))
    }
}

impl<'de, Context, T: BorrowDecode<'de, Context>> BorrowDecode<'de, Context> for Reverse<T> {
    fn borrow_decode<D: BorrowDecoder<'de, Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        Ok(Reverse(T::borrow_decode(decoder)?))
    }

    const BORROW_DECODE_NICHES: NicheTable = T::BORROW_DECODE_NICHES;

    fn borrow_decode_with_first_byte<D: BorrowDecoder<'de, Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        Ok(Reverse(T::borrow_decode_with_first_byte(first, decoder)?))
    }
}

impl<Context> Decode<Context> for char {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        // Look at the first byte to see how many bytes must be read
        let first = u8::decode(decoder)?;
        Self::decode_with_first_byte(first, decoder)
    }

    const DECODE_NICHES: NicheTable = NicheTable::uniform(Niches::range(0x80, 0xC1));

    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let mut array = [first, 0, 0, 0];

        let width = utf8_char_width(first);
        if width == 0 {
            return Err(DecodeError::InvalidCharEncoding(array));
        }
        if width == 1 {
            return Ok(first as char);
        }
        // The first byte has already been claimed.
        decoder.claim_bytes_read(width - 1)?;

        // read the remaining pain
        decoder.reader().read(&mut array[1..width])?;
        let res = core::str::from_utf8(&array[..width])
            .ok()
            .and_then(|s| s.chars().next())
            .ok_or(DecodeError::InvalidCharEncoding(array))?;
        Ok(res)
    }
}
impl_borrow_decode!(char);

impl<'a, 'de: 'a, Context> BorrowDecode<'de, Context> for &'a [u8] {
    fn borrow_decode<D: BorrowDecoder<'de, Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let len = super::decode_slice_len(decoder)?;
        decoder.claim_bytes_read(len)?;
        decoder.borrow_reader().take_bytes(len)
    }

    const BORROW_DECODE_NICHES: NicheTable = SLICE_LEN_NICHES;

    fn borrow_decode_with_first_byte<D: BorrowDecoder<'de, Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let len = super::decode_slice_len_with_first_byte(first, decoder)?;
        decoder.claim_bytes_read(len)?;
        decoder.borrow_reader().take_bytes(len)
    }
}

impl<'a, 'de: 'a, Context> BorrowDecode<'de, Context> for &'a str {
    fn borrow_decode<D: BorrowDecoder<'de, Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let slice = <&[u8]>::borrow_decode(decoder)?;
        core::str::from_utf8(slice).map_err(|inner| DecodeError::Utf8 { inner })
    }

    const BORROW_DECODE_NICHES: NicheTable =
        <&'a [u8] as BorrowDecode<'de, Context>>::BORROW_DECODE_NICHES;

    fn borrow_decode_with_first_byte<D: BorrowDecoder<'de, Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let slice = <&[u8]>::borrow_decode_with_first_byte(first, decoder)?;
        core::str::from_utf8(slice).map_err(|inner| DecodeError::Utf8 { inner })
    }
}

impl<Context, T, const N: usize> Decode<Context> for [T; N]
where
    T: Decode<Context>,
{
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        decoder.claim_bytes_read(core::mem::size_of::<[T; N]>())?;

        if unty::type_equal::<T, u8>() {
            let mut buf = [0u8; N];
            decoder.reader().read(&mut buf)?;
            let ptr = &mut buf as *mut _ as *mut [T; N];

            // Safety: we know that T is a u8, so it is perfectly safe to
            // translate an array of u8 into an array of T
            let res = unsafe { ptr.read() };
            Ok(res)
        } else {
            let result = super::impl_core::collect_into_array(&mut (0..N).map(|_| {
                // See the documentation on `unclaim_bytes_read` as to why we're doing this here
                decoder.unclaim_bytes_read(core::mem::size_of::<T>());
                T::decode(decoder)
            }));

            // result is only None if N does not match the values of `(0..N)`, which it always should
            // So this unwrap should never occur
            result.unwrap()
        }
    }

    const DECODE_NICHES: NicheTable = if N == 0 {
        NicheTable::NONE
    } else {
        T::DECODE_NICHES
    };

    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        // Claim and unclaim the same amounts as `decode`, less the first byte, which has already
        // been claimed.
        decoder.claim_bytes_read(core::mem::size_of::<[T; N]>().saturating_sub(1))?;

        let result = super::impl_core::collect_into_array(&mut (0..N).map(|i| {
            // See the documentation on `unclaim_bytes_read` as to why we're doing this here
            if i == 0 {
                decoder.unclaim_bytes_read(core::mem::size_of::<T>().saturating_sub(1));
                T::decode_with_first_byte(first, decoder)
            } else {
                decoder.unclaim_bytes_read(core::mem::size_of::<T>());
                T::decode(decoder)
            }
        }));

        // result is only None if N does not match the values of `(0..N)`, which it always should
        // So this unwrap should never occur
        result.unwrap()
    }
}

impl<'de, T, const N: usize, Context> BorrowDecode<'de, Context> for [T; N]
where
    T: BorrowDecode<'de, Context>,
{
    fn borrow_decode<D: BorrowDecoder<'de, Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        decoder.claim_bytes_read(core::mem::size_of::<[T; N]>())?;

        if unty::type_equal::<T, u8>() {
            let mut buf = [0u8; N];
            decoder.reader().read(&mut buf)?;
            let ptr = &mut buf as *mut _ as *mut [T; N];

            // Safety: we know that T is a u8, so it is perfectly safe to
            // translate an array of u8 into an array of T
            let res = unsafe { ptr.read() };
            Ok(res)
        } else {
            let result = super::impl_core::collect_into_array(&mut (0..N).map(|_| {
                // See the documentation on `unclaim_bytes_read` as to why we're doing this here
                decoder.unclaim_bytes_read(core::mem::size_of::<T>());
                T::borrow_decode(decoder)
            }));

            // result is only None if N does not match the values of `(0..N)`, which it always should
            // So this unwrap should never occur
            result.unwrap()
        }
    }

    const BORROW_DECODE_NICHES: NicheTable = if N == 0 {
        NicheTable::NONE
    } else {
        T::BORROW_DECODE_NICHES
    };

    fn borrow_decode_with_first_byte<D: BorrowDecoder<'de, Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        // Claim and unclaim the same amounts as `decode`, less the first byte, which has already
        // been claimed.
        decoder.claim_bytes_read(core::mem::size_of::<[T; N]>().saturating_sub(1))?;

        let result = super::impl_core::collect_into_array(&mut (0..N).map(|i| {
            // See the documentation on `unclaim_bytes_read` as to why we're doing this here
            if i == 0 {
                decoder.unclaim_bytes_read(core::mem::size_of::<T>().saturating_sub(1));
                T::borrow_decode_with_first_byte(first, decoder)
            } else {
                decoder.unclaim_bytes_read(core::mem::size_of::<T>());
                T::borrow_decode(decoder)
            }
        }));

        // result is only None if N does not match the values of `(0..N)`, which it always should
        // So this unwrap should never occur
        result.unwrap()
    }
}

impl<Context> Decode<Context> for () {
    fn decode<D: Decoder<Context = Context>>(_: &mut D) -> Result<Self, DecodeError> {
        Ok(())
    }
}
impl_borrow_decode!(());

impl<Context, T: ?Sized> Decode<Context> for core::marker::PhantomData<T> {
    fn decode<D: Decoder<Context = Context>>(_: &mut D) -> Result<Self, DecodeError> {
        Ok(core::marker::PhantomData)
    }
}
impl_borrow_decode!(core::marker::PhantomData<T>, T: ?Sized);

impl<Context, T> Decode<Context> for Option<T>
where
    T: Decode<Context>,
{
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let first = u8::decode(decoder)?;
        Self::decode_with_first_byte(first, decoder)
    }

    const DECODE_NICHES: NicheTable = T::DECODE_NICHES.option();

    #[inline]
    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        match super::decode_option_first_byte(
            first,
            const { T::DECODE_NICHES.get::<D::C>() },
            core::any::type_name::<Option<T>>(),
        )? {
            OptionFirstByte::None => Ok(None),
            OptionFirstByte::SomeTag => Ok(Some(T::decode(decoder)?)),
            OptionFirstByte::SomePayload => Ok(Some(T::decode_with_first_byte(first, decoder)?)),
        }
    }
}

impl<'de, T, Context> BorrowDecode<'de, Context> for Option<T>
where
    T: BorrowDecode<'de, Context>,
{
    fn borrow_decode<D: BorrowDecoder<'de, Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let first = u8::decode(decoder)?;
        Self::borrow_decode_with_first_byte(first, decoder)
    }

    const BORROW_DECODE_NICHES: NicheTable = T::BORROW_DECODE_NICHES.option();

    #[inline]
    fn borrow_decode_with_first_byte<D: BorrowDecoder<'de, Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        match super::decode_option_first_byte(
            first,
            const { T::BORROW_DECODE_NICHES.get::<D::C>() },
            core::any::type_name::<Option<T>>(),
        )? {
            OptionFirstByte::None => Ok(None),
            OptionFirstByte::SomeTag => Ok(Some(T::borrow_decode(decoder)?)),
            OptionFirstByte::SomePayload => {
                Ok(Some(T::borrow_decode_with_first_byte(first, decoder)?))
            }
        }
    }
}

impl<Context, T, U> Decode<Context> for Result<T, U>
where
    T: Decode<Context>,
    U: Decode<Context>,
{
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let is_ok = u32::decode(decoder)?;
        decode_result_variant(is_ok, decoder)
    }

    const DECODE_NICHES: NicheTable = NicheTable::variant_index(2);

    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let is_ok = u32::decode_with_first_byte(first, decoder)?;
        decode_result_variant(is_ok, decoder)
    }
}

/// Decodes the variant of a `Result<T, U>` with index `is_ok`.
fn decode_result_variant<Context, T, U, D: Decoder<Context = Context>>(
    is_ok: u32,
    decoder: &mut D,
) -> Result<Result<T, U>, DecodeError>
where
    T: Decode<Context>,
    U: Decode<Context>,
{
    match is_ok {
        0 => {
            let t = T::decode(decoder)?;
            Ok(Ok(t))
        }
        1 => {
            let u = U::decode(decoder)?;
            Ok(Err(u))
        }
        x => Err(DecodeError::UnexpectedVariant {
            found: x,
            allowed: &crate::error::AllowedEnumVariants::Range { max: 1, min: 0 },
            type_name: core::any::type_name::<Result<T, U>>(),
        }),
    }
}

impl<'de, T, U, Context> BorrowDecode<'de, Context> for Result<T, U>
where
    T: BorrowDecode<'de, Context>,
    U: BorrowDecode<'de, Context>,
{
    fn borrow_decode<D: BorrowDecoder<'de, Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let is_ok = u32::decode(decoder)?;
        borrow_decode_result_variant(is_ok, decoder)
    }

    const BORROW_DECODE_NICHES: NicheTable = NicheTable::variant_index(2);

    fn borrow_decode_with_first_byte<D: BorrowDecoder<'de, Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let is_ok = u32::decode_with_first_byte(first, decoder)?;
        borrow_decode_result_variant(is_ok, decoder)
    }
}

/// Decodes the variant of a `Result<T, U>` with index `is_ok`.
fn borrow_decode_result_variant<'de, T, U, Context, D: BorrowDecoder<'de, Context = Context>>(
    is_ok: u32,
    decoder: &mut D,
) -> Result<Result<T, U>, DecodeError>
where
    T: BorrowDecode<'de, Context>,
    U: BorrowDecode<'de, Context>,
{
    match is_ok {
        0 => {
            let t = T::borrow_decode(decoder)?;
            Ok(Ok(t))
        }
        1 => {
            let u = U::borrow_decode(decoder)?;
            Ok(Err(u))
        }
        x => Err(DecodeError::UnexpectedVariant {
            found: x,
            allowed: &crate::error::AllowedEnumVariants::Range { max: 1, min: 0 },
            type_name: core::any::type_name::<Result<T, U>>(),
        }),
    }
}

impl<Context, T> Decode<Context> for Cell<T>
where
    T: Decode<Context>,
{
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let t = T::decode(decoder)?;
        Ok(Cell::new(t))
    }

    const DECODE_NICHES: NicheTable = T::DECODE_NICHES;

    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        Ok(Cell::new(T::decode_with_first_byte(first, decoder)?))
    }
}

impl<'de, T, Context> BorrowDecode<'de, Context> for Cell<T>
where
    T: BorrowDecode<'de, Context>,
{
    fn borrow_decode<D: BorrowDecoder<'de, Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let t = T::borrow_decode(decoder)?;
        Ok(Cell::new(t))
    }

    const BORROW_DECODE_NICHES: NicheTable = T::BORROW_DECODE_NICHES;

    fn borrow_decode_with_first_byte<D: BorrowDecoder<'de, Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        Ok(Cell::new(T::borrow_decode_with_first_byte(first, decoder)?))
    }
}

impl<Context, T> Decode<Context> for RefCell<T>
where
    T: Decode<Context>,
{
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let t = T::decode(decoder)?;
        Ok(RefCell::new(t))
    }

    const DECODE_NICHES: NicheTable = T::DECODE_NICHES;

    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        Ok(RefCell::new(T::decode_with_first_byte(first, decoder)?))
    }
}

impl<'de, T, Context> BorrowDecode<'de, Context> for RefCell<T>
where
    T: BorrowDecode<'de, Context>,
{
    fn borrow_decode<D: BorrowDecoder<'de, Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let t = T::borrow_decode(decoder)?;
        Ok(RefCell::new(t))
    }

    const BORROW_DECODE_NICHES: NicheTable = T::BORROW_DECODE_NICHES;

    fn borrow_decode_with_first_byte<D: BorrowDecoder<'de, Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        Ok(RefCell::new(T::borrow_decode_with_first_byte(
            first, decoder,
        )?))
    }
}

impl<Context> Decode<Context> for Duration {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let secs: u64 = Decode::decode(decoder)?;
        let nanos: u32 = Decode::decode(decoder)?;
        duration_from_parts(secs, nanos)
    }

    const DECODE_NICHES: NicheTable = <u64 as Decode<Context>>::DECODE_NICHES;

    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let secs = u64::decode_with_first_byte(first, decoder)?;
        let nanos: u32 = Decode::decode(decoder)?;
        duration_from_parts(secs, nanos)
    }
}

fn duration_from_parts(secs: u64, nanos: u32) -> Result<Duration, DecodeError> {
    const NANOS_PER_SEC: u64 = 1_000_000_000;
    if secs.checked_add(u64::from(nanos) / NANOS_PER_SEC).is_none() {
        return Err(DecodeError::InvalidDuration { secs, nanos });
    }
    Ok(Duration::new(secs, nanos))
}
impl_borrow_decode!(Duration);

impl<Context, T> Decode<Context> for Range<T>
where
    T: Decode<Context>,
{
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let min = T::decode(decoder)?;
        let max = T::decode(decoder)?;
        Ok(min..max)
    }

    const DECODE_NICHES: NicheTable = T::DECODE_NICHES;

    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let min = T::decode_with_first_byte(first, decoder)?;
        let max = T::decode(decoder)?;
        Ok(min..max)
    }
}
impl<'de, T, Context> BorrowDecode<'de, Context> for Range<T>
where
    T: BorrowDecode<'de, Context>,
{
    fn borrow_decode<D: BorrowDecoder<'de, Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let min = T::borrow_decode(decoder)?;
        let max = T::borrow_decode(decoder)?;
        Ok(min..max)
    }

    const BORROW_DECODE_NICHES: NicheTable = T::BORROW_DECODE_NICHES;

    fn borrow_decode_with_first_byte<D: BorrowDecoder<'de, Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let min = T::borrow_decode_with_first_byte(first, decoder)?;
        let max = T::borrow_decode(decoder)?;
        Ok(min..max)
    }
}

impl<Context, T> Decode<Context> for RangeInclusive<T>
where
    T: Decode<Context>,
{
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let min = T::decode(decoder)?;
        let max = T::decode(decoder)?;
        Ok(RangeInclusive::new(min, max))
    }

    const DECODE_NICHES: NicheTable = T::DECODE_NICHES;

    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let min = T::decode_with_first_byte(first, decoder)?;
        let max = T::decode(decoder)?;
        Ok(RangeInclusive::new(min, max))
    }
}

impl<'de, T, Context> BorrowDecode<'de, Context> for RangeInclusive<T>
where
    T: BorrowDecode<'de, Context>,
{
    fn borrow_decode<D: BorrowDecoder<'de, Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let min = T::borrow_decode(decoder)?;
        let max = T::borrow_decode(decoder)?;
        Ok(RangeInclusive::new(min, max))
    }

    const BORROW_DECODE_NICHES: NicheTable = T::BORROW_DECODE_NICHES;

    fn borrow_decode_with_first_byte<D: BorrowDecoder<'de, Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let min = T::borrow_decode_with_first_byte(first, decoder)?;
        let max = T::borrow_decode(decoder)?;
        Ok(RangeInclusive::new(min, max))
    }
}

impl<T, Context> Decode<Context> for Bound<T>
where
    T: Decode<Context>,
{
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let variant = u32::decode(decoder)?;
        decode_bound_variant(variant, decoder)
    }

    const DECODE_NICHES: NicheTable = NicheTable::variant_index(3);

    fn decode_with_first_byte<D: Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let variant = u32::decode_with_first_byte(first, decoder)?;
        decode_bound_variant(variant, decoder)
    }
}

/// Decodes the variant of a `Bound<T>` with index `variant`.
fn decode_bound_variant<T, Context, D: Decoder<Context = Context>>(
    variant: u32,
    decoder: &mut D,
) -> Result<Bound<T>, DecodeError>
where
    T: Decode<Context>,
{
    match variant {
        0 => Ok(Bound::Unbounded),
        1 => Ok(Bound::Included(T::decode(decoder)?)),
        2 => Ok(Bound::Excluded(T::decode(decoder)?)),
        x => Err(DecodeError::UnexpectedVariant {
            allowed: &crate::error::AllowedEnumVariants::Range { max: 2, min: 0 },
            found: x,
            type_name: core::any::type_name::<Bound<T>>(),
        }),
    }
}

impl<'de, T, Context> BorrowDecode<'de, Context> for Bound<T>
where
    T: BorrowDecode<'de, Context>,
{
    fn borrow_decode<D: BorrowDecoder<'de, Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let variant = u32::decode(decoder)?;
        borrow_decode_bound_variant(variant, decoder)
    }

    const BORROW_DECODE_NICHES: NicheTable = NicheTable::variant_index(3);

    fn borrow_decode_with_first_byte<D: BorrowDecoder<'de, Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, DecodeError> {
        let variant = u32::decode_with_first_byte(first, decoder)?;
        borrow_decode_bound_variant(variant, decoder)
    }
}

/// Decodes the variant of a `Bound<T>` with index `variant`.
fn borrow_decode_bound_variant<'de, T, Context, D: BorrowDecoder<'de, Context = Context>>(
    variant: u32,
    decoder: &mut D,
) -> Result<Bound<T>, DecodeError>
where
    T: BorrowDecode<'de, Context>,
{
    match variant {
        0 => Ok(Bound::Unbounded),
        1 => Ok(Bound::Included(T::borrow_decode(decoder)?)),
        2 => Ok(Bound::Excluded(T::borrow_decode(decoder)?)),
        x => Err(DecodeError::UnexpectedVariant {
            allowed: &crate::error::AllowedEnumVariants::Range { max: 2, min: 0 },
            found: x,
            type_name: core::any::type_name::<Bound<T>>(),
        }),
    }
}

const UTF8_CHAR_WIDTH: [u8; 256] = [
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, // 0x1F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, // 0x3F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, // 0x5F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, // 0x7F
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, // 0x9F
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, // 0xBF
    0, 0, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, // 0xDF
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, // 0xEF
    4, 4, 4, 4, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 0xFF
];

// This function is a copy of core::str::utf8_char_width
const fn utf8_char_width(b: u8) -> usize {
    UTF8_CHAR_WIDTH[b as usize] as usize
}
