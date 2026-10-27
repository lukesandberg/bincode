use crate::{de::Decode, enc::Encode, impl_borrow_decode, niche::NicheTable};
use core::sync::atomic::Ordering;

#[cfg(target_has_atomic = "ptr")]
use core::sync::atomic::{AtomicIsize, AtomicUsize};

#[cfg(target_has_atomic = "8")]
use core::sync::atomic::{AtomicBool, AtomicI8, AtomicU8};

#[cfg(target_has_atomic = "16")]
use core::sync::atomic::{AtomicI16, AtomicU16};

#[cfg(target_has_atomic = "32")]
use core::sync::atomic::{AtomicI32, AtomicU32};

#[cfg(target_has_atomic = "64")]
use core::sync::atomic::{AtomicI64, AtomicU64};

#[cfg(target_has_atomic = "8")]
impl Encode for AtomicBool {
    fn encode<E: crate::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), crate::error::EncodeError> {
        self.load(Ordering::SeqCst).encode(encoder)
    }

    const ENCODE_NICHES: NicheTable = bool::ENCODE_NICHES;
}

#[cfg(target_has_atomic = "8")]
impl<Context> Decode<Context> for AtomicBool {
    fn decode<D: crate::de::Decoder>(decoder: &mut D) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicBool::new(Decode::decode(decoder)?))
    }

    const DECODE_NICHES: NicheTable = <bool as Decode<Context>>::DECODE_NICHES;

    fn decode_with_first_byte<D: crate::de::Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicBool::new(bool::decode_with_first_byte(
            first, decoder,
        )?))
    }
}
#[cfg(target_has_atomic = "8")]
impl_borrow_decode!(AtomicBool);

#[cfg(target_has_atomic = "8")]
impl Encode for AtomicU8 {
    fn encode<E: crate::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), crate::error::EncodeError> {
        self.load(Ordering::SeqCst).encode(encoder)
    }
}

#[cfg(target_has_atomic = "8")]
impl<Context> Decode<Context> for AtomicU8 {
    fn decode<D: crate::de::Decoder>(decoder: &mut D) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicU8::new(Decode::decode(decoder)?))
    }
}
#[cfg(target_has_atomic = "8")]
impl_borrow_decode!(AtomicU8);

#[cfg(target_has_atomic = "16")]
impl Encode for AtomicU16 {
    fn encode<E: crate::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), crate::error::EncodeError> {
        self.load(Ordering::SeqCst).encode(encoder)
    }

    const ENCODE_NICHES: NicheTable = u16::ENCODE_NICHES;
}

#[cfg(target_has_atomic = "16")]
impl<Context> Decode<Context> for AtomicU16 {
    fn decode<D: crate::de::Decoder>(decoder: &mut D) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicU16::new(Decode::decode(decoder)?))
    }

    const DECODE_NICHES: NicheTable = <u16 as Decode<Context>>::DECODE_NICHES;

    fn decode_with_first_byte<D: crate::de::Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicU16::new(u16::decode_with_first_byte(first, decoder)?))
    }
}
#[cfg(target_has_atomic = "16")]
impl_borrow_decode!(AtomicU16);

#[cfg(target_has_atomic = "32")]
impl Encode for AtomicU32 {
    fn encode<E: crate::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), crate::error::EncodeError> {
        self.load(Ordering::SeqCst).encode(encoder)
    }

    const ENCODE_NICHES: NicheTable = u32::ENCODE_NICHES;
}

#[cfg(target_has_atomic = "32")]
impl<Context> Decode<Context> for AtomicU32 {
    fn decode<D: crate::de::Decoder>(decoder: &mut D) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicU32::new(Decode::decode(decoder)?))
    }

    const DECODE_NICHES: NicheTable = <u32 as Decode<Context>>::DECODE_NICHES;

    fn decode_with_first_byte<D: crate::de::Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicU32::new(u32::decode_with_first_byte(first, decoder)?))
    }
}
#[cfg(target_has_atomic = "32")]
impl_borrow_decode!(AtomicU32);

#[cfg(target_has_atomic = "64")]
impl Encode for AtomicU64 {
    fn encode<E: crate::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), crate::error::EncodeError> {
        self.load(Ordering::SeqCst).encode(encoder)
    }

    const ENCODE_NICHES: NicheTable = u64::ENCODE_NICHES;
}

#[cfg(target_has_atomic = "64")]
impl<Context> Decode<Context> for AtomicU64 {
    fn decode<D: crate::de::Decoder>(decoder: &mut D) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicU64::new(Decode::decode(decoder)?))
    }

    const DECODE_NICHES: NicheTable = <u64 as Decode<Context>>::DECODE_NICHES;

    fn decode_with_first_byte<D: crate::de::Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicU64::new(u64::decode_with_first_byte(first, decoder)?))
    }
}
#[cfg(target_has_atomic = "64")]
impl_borrow_decode!(AtomicU64);

#[cfg(target_has_atomic = "ptr")]
impl Encode for AtomicUsize {
    fn encode<E: crate::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), crate::error::EncodeError> {
        self.load(Ordering::SeqCst).encode(encoder)
    }

    const ENCODE_NICHES: NicheTable = usize::ENCODE_NICHES;
}

#[cfg(target_has_atomic = "ptr")]
impl<Context> Decode<Context> for AtomicUsize {
    fn decode<D: crate::de::Decoder>(decoder: &mut D) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicUsize::new(Decode::decode(decoder)?))
    }

    const DECODE_NICHES: NicheTable = <usize as Decode<Context>>::DECODE_NICHES;

    fn decode_with_first_byte<D: crate::de::Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicUsize::new(usize::decode_with_first_byte(
            first, decoder,
        )?))
    }
}
#[cfg(target_has_atomic = "ptr")]
impl_borrow_decode!(AtomicUsize);

#[cfg(target_has_atomic = "8")]
impl Encode for AtomicI8 {
    fn encode<E: crate::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), crate::error::EncodeError> {
        self.load(Ordering::SeqCst).encode(encoder)
    }
}

#[cfg(target_has_atomic = "8")]
impl<Context> Decode<Context> for AtomicI8 {
    fn decode<D: crate::de::Decoder>(decoder: &mut D) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicI8::new(Decode::decode(decoder)?))
    }
}
#[cfg(target_has_atomic = "8")]
impl_borrow_decode!(AtomicI8);

#[cfg(target_has_atomic = "16")]
impl Encode for AtomicI16 {
    fn encode<E: crate::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), crate::error::EncodeError> {
        self.load(Ordering::SeqCst).encode(encoder)
    }

    const ENCODE_NICHES: NicheTable = i16::ENCODE_NICHES;
}

#[cfg(target_has_atomic = "16")]
impl<Context> Decode<Context> for AtomicI16 {
    fn decode<D: crate::de::Decoder>(decoder: &mut D) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicI16::new(Decode::decode(decoder)?))
    }

    const DECODE_NICHES: NicheTable = <i16 as Decode<Context>>::DECODE_NICHES;

    fn decode_with_first_byte<D: crate::de::Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicI16::new(i16::decode_with_first_byte(first, decoder)?))
    }
}
#[cfg(target_has_atomic = "16")]
impl_borrow_decode!(AtomicI16);

#[cfg(target_has_atomic = "32")]
impl Encode for AtomicI32 {
    fn encode<E: crate::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), crate::error::EncodeError> {
        self.load(Ordering::SeqCst).encode(encoder)
    }

    const ENCODE_NICHES: NicheTable = i32::ENCODE_NICHES;
}

#[cfg(target_has_atomic = "32")]
impl<Context> Decode<Context> for AtomicI32 {
    fn decode<D: crate::de::Decoder>(decoder: &mut D) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicI32::new(Decode::decode(decoder)?))
    }

    const DECODE_NICHES: NicheTable = <i32 as Decode<Context>>::DECODE_NICHES;

    fn decode_with_first_byte<D: crate::de::Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicI32::new(i32::decode_with_first_byte(first, decoder)?))
    }
}
#[cfg(target_has_atomic = "32")]
impl_borrow_decode!(AtomicI32);

#[cfg(target_has_atomic = "64")]
impl Encode for AtomicI64 {
    fn encode<E: crate::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), crate::error::EncodeError> {
        self.load(Ordering::SeqCst).encode(encoder)
    }

    const ENCODE_NICHES: NicheTable = i64::ENCODE_NICHES;
}

#[cfg(target_has_atomic = "64")]
impl<Context> Decode<Context> for AtomicI64 {
    fn decode<D: crate::de::Decoder>(decoder: &mut D) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicI64::new(Decode::decode(decoder)?))
    }

    const DECODE_NICHES: NicheTable = <i64 as Decode<Context>>::DECODE_NICHES;

    fn decode_with_first_byte<D: crate::de::Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicI64::new(i64::decode_with_first_byte(first, decoder)?))
    }
}
#[cfg(target_has_atomic = "64")]
impl_borrow_decode!(AtomicI64);

#[cfg(target_has_atomic = "ptr")]
impl Encode for AtomicIsize {
    fn encode<E: crate::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), crate::error::EncodeError> {
        self.load(Ordering::SeqCst).encode(encoder)
    }

    const ENCODE_NICHES: NicheTable = isize::ENCODE_NICHES;
}

#[cfg(target_has_atomic = "ptr")]
impl<Context> Decode<Context> for AtomicIsize {
    fn decode<D: crate::de::Decoder>(decoder: &mut D) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicIsize::new(Decode::decode(decoder)?))
    }

    const DECODE_NICHES: NicheTable = <isize as Decode<Context>>::DECODE_NICHES;

    fn decode_with_first_byte<D: crate::de::Decoder<Context = Context>>(
        first: u8,
        decoder: &mut D,
    ) -> Result<Self, crate::error::DecodeError> {
        Ok(AtomicIsize::new(isize::decode_with_first_byte(
            first, decoder,
        )?))
    }
}
#[cfg(target_has_atomic = "ptr")]
impl_borrow_decode!(AtomicIsize);
