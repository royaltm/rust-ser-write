//! MessagePack serde deserializer

// use std::println;
#[cfg(feature = "std")]
use std::{string::{String, ToString}};

#[cfg(all(feature = "alloc",not(feature = "std")))]
use alloc::{string::{String, ToString}};

use core::convert::Infallible;
use core::num::{NonZeroUsize, TryFromIntError};
use core::str::{Utf8Error, FromStr};
use core::{fmt, str};
use serde::de::{self, Visitor, SeqAccess, MapAccess, DeserializeSeed};

use crate::magick::*;

/// Deserialize an instance of type `T` from a slice of bytes in a MessagePack format.
///
/// Return a tuple with `(value, msgpack_len)`. `msgpack_len` <= `input.len()`.
///
/// Any `&str` or `&[u8]` in the returned type will contain references to the provided slice.
pub fn from_slice<'a, T>(input: &'a[u8]) -> Result<(T, usize)>
    where T: de::Deserialize<'a>
{
    let mut de = Deserializer::from_slice(input);
    let value = de::Deserialize::deserialize(&mut de)?;
    let tail_len = de.end()?;

    Ok((value, input.len() - tail_len))
}

/// Deserialize an instance of type `T` from a slice of bytes in a MessagePack format.
///
/// Return a tuple with `(value, tail)`, where `tail` is the tail of the input beginning
/// at the byte following the last byte of the serialized data.
///
/// Any `&str` or `&[u8]` in the returned type will contain references to the provided slice.
pub fn from_slice_split_tail<'a, T>(input: &'a[u8]) -> Result<(T, &'a[u8])>
    where T: de::Deserialize<'a>
{
    let (value, len) = from_slice(input)?;
    Ok((value, &input[len..]))
}

/// Serde MessagePack deserializer.
///
/// * deserializes data from a slice,
/// * deserializes borrowed references to `&str` and `&[u8]` types,
/// * deserializes structs from MessagePack maps or arrays.
/// * deserializes enum variants and struct fields from MessagePack strings or integers.
/// * deserializes integers from any MessagePack integer type as long as the number can be casted safely
/// * deserializes floats from any MessagePack integer or float types
/// * deserializes floats as `NaN` from `nil`
pub struct Deserializer<'de> {
    input: &'de[u8],
    index: usize
}

/// Deserialization result
pub type Result<T> = core::result::Result<T, Error>;

/// Deserialization error
#[derive(Debug, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub enum Error {
    /// EOF while parsing
    UnexpectedEof,
    /// Reserved code was detected
    ReservedCode,
    /// Unsopported extension was detected
    UnsupportedExt,
    /// Number could not be coerced
    InvalidInteger,
    /// Invalid type
    InvalidType,
    /// Invalid unicode code point
    InvalidUnicodeCodePoint,
    /// Expected an integer type
    ExpectedInteger,
    /// Expected a number type
    ExpectedNumber,
    /// Expected a string
    ExpectedString,
    /// Expected a binary type
    ExpectedBin,
    /// Expected NIL type
    ExpectedNil,
    /// Expected an array type
    ExpectedArray,
    /// Expected a map type
    ExpectedMap,
    /// Expected a map or an array type
    ExpectedStruct,
    /// Expected struct or variant identifier
    ExpectedIdentifier,
    /// Trailing unserialized array elements
    TrailingElements,
    /// Invalid length
    InvalidLength,
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
    /// An error passed down from a [`serde::de::Deserialize`] implementation
    DeserializeError(String),
    #[cfg(not(any(feature = "std", feature = "alloc")))]
    DeserializeError
}

impl serde::de::StdError for Error {}

#[cfg(any(feature = "std", feature = "alloc"))]
impl de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::DeserializeError(msg.to_string())
    }
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl de::Error for Error {
    fn custom<T: fmt::Display>(_msg: T) -> Self {
        Error::DeserializeError
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Error::UnexpectedEof => "Unexpected end of MessagePack input",
            Error::ReservedCode => "Reserved MessagePack code in input",
            Error::UnsupportedExt => "Unsupported MessagePack extension code in input",
            Error::InvalidInteger => "Could not coerce integer to a deserialized type",
            Error::InvalidType => "Invalid type",
            Error::InvalidUnicodeCodePoint => "Invalid unicode code point",
            Error::ExpectedInteger => "Expected MessagePack integer",
            Error::ExpectedNumber => "Expected MessagePack number",
            Error::ExpectedString => "Expected MessagePack string",
            Error::ExpectedBin => "Expected MessagePack bin",
            Error::ExpectedNil => "Expected MessagePack nil",
            Error::ExpectedArray => "Expected MessagePack array",
            Error::ExpectedMap => "Expected MessagePack map",
            Error::ExpectedStruct => "Expected MessagePack map or array",
            Error::ExpectedIdentifier => "Expected a struct field or enum variant identifier",
            Error::TrailingElements => "Too many elements for a deserialized type",
            Error::InvalidLength => "Invalid length",
            #[cfg(any(feature = "std", feature = "alloc"))]
            Error::DeserializeError(s) => return write!(f, "{} while deserializing MessagePack", s),
            #[cfg(not(any(feature = "std", feature = "alloc")))]
            Error::DeserializeError => "MessagePack does not match deserializer’s expected format",
        })
    }
}

impl From<TryFromIntError> for Error {
    fn from(_err: TryFromIntError) -> Self {
        Error::InvalidInteger
    }
}

impl From<Infallible> for Error {
    fn from(_err: Infallible) -> Self {
        unreachable!()
    }
}

impl From<Utf8Error> for Error {
    fn from(_err: Utf8Error) -> Self {
        Error::InvalidUnicodeCodePoint
    }
}

enum MsgType {
    Single(usize),
    Array(usize),
    Map(usize),
}

/// Some methods in a `Deserializer` object are made public to allow custom
/// manipulation of MessagePack encoded data for other purposes than simply
/// deserializing.
///
/// For example, splitting a stream of messages encoded with the MessagePack
/// format without fully decoding messages.
impl<'de> Deserializer<'de> {
    /// Create a new decoder instance by providing a slice from which to
    /// deserialize messages.
    pub fn from_slice(input: &'de[u8]) -> Self {
        Deserializer { input, index: 0, }
    }
    /// Consume [`Deserializer`] and return the number of unparsed bytes in
    /// the input slice on success.
    ///
    /// If the input cursor points outside the input slice, an error
    /// `Error::UnexpectedEof` is returned.
    pub fn end(self) -> Result<usize> {
        self.input.len()
        .checked_sub(self.index)
        .ok_or(Error::UnexpectedEof)
    }
    /// Return the remaining number of unparsed bytes in the input slice.
    ///
    /// Returns 0 when the input cursor points either at the end or beyond
    /// the end of the input slice.
    #[inline]
    pub fn remaining_len(&self) -> usize {
        self.input.len().saturating_sub(self.index)
    }
    /// Peek at the next byte code and return it on success, otherwise return
    /// `Err(Error::UnexpectedEof)` if there are no more unparsed bytes
    /// remaining in the input slice.
    #[inline]
    pub fn peek(&self) -> Result<u8> {
        self.input.get(self.index).copied()
        .ok_or(Error::UnexpectedEof)
    }
    /// Advance the input cursor by `len` bytes.
    ///
    /// _Note_: this function only increases a cursor without any checks!
    #[inline(always)]
    pub fn eat_some(&mut self, len: usize) {
        self.index += len;
    }
    /// Return a reference to the unparsed portion of the input slice on success.
    ///
    /// If the input cursor points outside the input slice, an error
    /// `Error::UnexpectedEof` is returned.
    #[inline]
    pub fn input_ref(&self) -> Result<&[u8]> {
        self.input.get(self.index..).ok_or(Error::UnexpectedEof)
    }
    /// Split the unparsed portion of the input slice between `0..len` and on success
    /// return it with the lifetime of the original slice container.
    ///
    /// The returned slice can be passed to `visit_borrowed_*` functions of a [`Visitor`].
    ///
    /// Drop already parsed bytes and the new unparsed input slice will begin at `len`.
    ///
    /// __Panics__ if `cursor` + `len` overflows `usize` integer capacity.
    pub fn split_input(&mut self, len: usize) -> Result<&'de[u8]> {
        let input = self.input.get(self.index..)
                    .ok_or(Error::UnexpectedEof)?;
        let (res, input) = input.split_at_checked(len)
                    .ok_or(Error::UnexpectedEof)?;
        self.input = input;
        self.index = 0;
        Ok(res)
    }
    /// Fetch the next byte from input or return an `Err::UnexpectedEof` error.
    pub fn fetch(&mut self) -> Result<u8> {
        let c = self.peek()?;
        self.eat_some(1);
        Ok(c)
    }

    fn fetch_array<const N: usize>(&mut self) -> Result<[u8;N]> {
        let index = self.index;
        let res = self.input.get(index..index+N)
        .ok_or(Error::UnexpectedEof)?
        .try_into().unwrap();
        self.eat_some(N);
        Ok(res)
    }

    fn fetch_u8(&mut self) -> Result<u8> {
        Ok(u8::from_be_bytes(self.fetch_array()?))
    }

    fn fetch_i8(&mut self) -> Result<i8> {
        Ok(i8::from_be_bytes(self.fetch_array()?))
    }

    fn fetch_u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.fetch_array()?))
    }

    fn fetch_i16(&mut self) -> Result<i16> {
        Ok(i16::from_be_bytes(self.fetch_array()?))
    }

    fn fetch_u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.fetch_array()?))
    }

    fn fetch_i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.fetch_array()?))
    }

    fn fetch_u64(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.fetch_array()?))
    }

    fn fetch_i64(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.fetch_array()?))
    }

    fn fetch_f32(&mut self) -> Result<f32> {
        Ok(f32::from_be_bytes(self.fetch_array()?))
    }

    fn fetch_f64(&mut self) -> Result<f64> {
        Ok(f64::from_be_bytes(self.fetch_array()?))
    }

    fn parse_str(&mut self) -> Result<&'de str> {
        let len: usize = match self.fetch()? {
            c@(FIXSTR..=FIXSTR_MAX) => (c as usize) & MAX_FIXSTR_SIZE,
            STR_8 => self.fetch_u8()?.into(),
            STR_16 => self.fetch_u16()?.into(),
            STR_32 => self.fetch_u32()?.try_into()?,
            _ => return Err(Error::ExpectedString)
        };
        Ok(core::str::from_utf8(self.split_input(len)?)?)
    }

    fn parse_bytes(&mut self) -> Result<&'de[u8]> {
        let len: usize = match self.fetch()? {
            BIN_8 => self.fetch_u8()?.into(),
            BIN_16 => self.fetch_u16()?.into(),
            BIN_32 => self.fetch_u32()?.try_into()?,
            _ => return Err(Error::ExpectedBin)
        };
        self.split_input(len)
    }

    fn parse_integer<N>(&mut self) -> Result<N>
        where N: TryFrom<i8> + TryFrom<u8> +
                 TryFrom<i16> + TryFrom<u16> +
                 TryFrom<i32> + TryFrom<u32> +
                 TryFrom<i64> + TryFrom<u64>,
              Error: From<<N as TryFrom<i8>>::Error>,
              Error: From<<N as TryFrom<u8>>::Error>,
              Error: From<<N as TryFrom<i16>>::Error>,
              Error: From<<N as TryFrom<u16>>::Error>,
              Error: From<<N as TryFrom<i32>>::Error>,
              Error: From<<N as TryFrom<u32>>::Error>,
              Error: From<<N as TryFrom<i64>>::Error>,
              Error: From<<N as TryFrom<u64>>::Error>,
    {
        let n: N = match self.fetch()? {
            n@(MIN_POSFIXINT..=MAX_POSFIXINT|NEGFIXINT..=0xff) => {
                (n as i8).try_into()?
            }
            UINT_8  => (self.fetch_u8()?).try_into()?,
            UINT_16 => (self.fetch_u16()?).try_into()?,
            UINT_32 => (self.fetch_u32()?).try_into()?,
            UINT_64 => (self.fetch_u64()?).try_into()?,
            INT_8   => (self.fetch_i8()?).try_into()?,
            INT_16  => (self.fetch_i16()?).try_into()?,
            INT_32  => (self.fetch_i32()?).try_into()?,
            INT_64  => (self.fetch_i64()?).try_into()?,
            _ => return Err(Error::ExpectedInteger)
        };
        Ok(n)
    }

    /// Attempts to consume a single MessagePack message from the input without fully decoding its content.
    ///
    /// Return `Ok(())` on success or `Err(Error::UnexpectedEof)` if there was not enough data
    /// to fully decode a MessagePack item.
    pub fn eat_message(&mut self) -> Result<()> {
        use MsgType::*;
        let mtyp = match self.fetch()? {
            NIL|
            FALSE|
            TRUE|
            MIN_POSFIXINT..=MAX_POSFIXINT|
            NEGFIXINT..=0xff => Single(0),
            c@(FIXMAP..=FIXMAP_MAX) => Map((c as usize) & MAX_FIXMAP_SIZE),
            c@(FIXARRAY..=FIXARRAY_MAX) => Array((c as usize) & MAX_FIXARRAY_SIZE),
            c@(FIXSTR..=FIXSTR_MAX) => Single((c as usize) & MAX_FIXSTR_SIZE),
            RESERVED => return Err(Error::ReservedCode),
            BIN_8|STR_8 => Single(self.fetch_u8()?.into()),
            BIN_16|STR_16 => Single(self.fetch_u16()?.into()),
            BIN_32|STR_32 => Single(self.fetch_u32()?.try_into()?),
            EXT_8 => Single(1usize + usize::from(self.fetch_u8()?)),
            EXT_16 => Single(1usize + usize::from(self.fetch_u16()?)),
            EXT_32 => Single(1usize + usize::try_from(self.fetch_u32()?)?),
            FLOAT_32 => Single(4),
            FLOAT_64 => Single(8),
            UINT_8 => Single(1),
            UINT_16 => Single(2),
            UINT_32 => Single(4),
            UINT_64 => Single(8),
            INT_8 => Single(1),
            INT_16 => Single(2),
            INT_32 => Single(4),
            INT_64 => Single(8),
            FIXEXT_1 => Single(2),
            FIXEXT_2 => Single(3),
            FIXEXT_4 => Single(5),
            FIXEXT_8 => Single(9),
            FIXEXT_16 => Single(17),
            ARRAY_16 => Array(self.fetch_u16()?.into()),
            ARRAY_32 => Array(self.fetch_u32()?.try_into()?),
            MAP_16 => Map(self.fetch_u16()?.into()),
            MAP_32 => Map(self.fetch_u32()?.try_into()?),
        };
        match mtyp {
            Single(len) => {
                let index = self.index + len;
                if index > self.input.len() {
                    return Err(Error::UnexpectedEof)
                }
                self.index = index;
            }
            Array(len) => self.eat_seq_items(len)?,
            Map(len) => self.eat_map_items(len)?
        }
        Ok(())
    }

    fn eat_seq_items(&mut self, len: usize) -> Result<()> {
        for _ in 0..len {
            self.eat_message()?;
        }
        Ok(())
    }

    fn eat_map_items(&mut self, len: usize) -> Result<()> {
        for _ in 0..len {
            self.eat_message()?;
            self.eat_message()?;
        }
        Ok(())
    }

}


impl<'de> de::Deserializer<'de> for &mut Deserializer<'de> {
    type Error = Error;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        match self.peek()? {
            MIN_POSFIXINT..=MAX_POSFIXINT => self.deserialize_u8(visitor),
            FIXMAP..=FIXMAP_MAX => self.deserialize_map(visitor),
            FIXARRAY..=FIXARRAY_MAX => self.deserialize_seq(visitor),
            FIXSTR..=FIXSTR_MAX => self.deserialize_str(visitor),
            NIL => self.deserialize_unit(visitor),
            RESERVED => Err(Error::ReservedCode),
            FALSE|
            TRUE => self.deserialize_bool(visitor),
            BIN_8|
            BIN_16|
            BIN_32 => self.deserialize_bytes(visitor),
            EXT_8|
            EXT_16|
            EXT_32 => Err(Error::UnsupportedExt),
            FLOAT_32 => self.deserialize_f32(visitor),
            FLOAT_64 => self.deserialize_f64(visitor),
            UINT_8 => self.deserialize_u8(visitor),
            UINT_16 => self.deserialize_u16(visitor),
            UINT_32 => self.deserialize_u32(visitor),
            UINT_64 => self.deserialize_u64(visitor),
            INT_8 => self.deserialize_i8(visitor),
            INT_16 => self.deserialize_i16(visitor),
            INT_32 => self.deserialize_i32(visitor),
            INT_64 => self.deserialize_i64(visitor),
            FIXEXT_1|
            FIXEXT_2|
            FIXEXT_4|
            FIXEXT_8|
            FIXEXT_16 => Err(Error::UnsupportedExt),
            STR_8|
            STR_16|
            STR_32 => self.deserialize_str(visitor),
            ARRAY_16|
            ARRAY_32 => self.deserialize_seq(visitor),
            MAP_16|
            MAP_32 => self.deserialize_map(visitor),
            NEGFIXINT..=0xff => self.deserialize_i8(visitor),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        let boolean = match self.fetch()? {
            TRUE => true,
            FALSE => false,
            _ => return Err(Error::InvalidType)
        };
        visitor.visit_bool(boolean)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_i8(self.parse_integer()?)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_i16(self.parse_integer()?)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_i32(self.parse_integer()?)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_i64(self.parse_integer()?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_u8(self.parse_integer()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_u16(self.parse_integer()?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_u32(self.parse_integer()?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_u64(self.parse_integer()?)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        let f: f32 = match self.fetch()? {
            FLOAT_32 => self.fetch_f32()?,
            FLOAT_64 => self.fetch_f64()? as f32,
            NIL => f32::NAN,
            n@(MIN_POSFIXINT..=MAX_POSFIXINT|NEGFIXINT..=0xff) => {
                (n as i8) as f32
            }
            UINT_8  => self.fetch_u8()?  as f32,
            UINT_16 => self.fetch_u16()? as f32,
            UINT_32 => self.fetch_u32()? as f32,
            UINT_64 => self.fetch_u64()? as f32,
            INT_8   => self.fetch_i8()?  as f32,
            INT_16  => self.fetch_i16()? as f32,
            INT_32  => self.fetch_i32()? as f32,
            INT_64  => self.fetch_i64()? as f32,
            _ => return Err(Error::ExpectedNumber)
        };
        visitor.visit_f32(f)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        let f: f64 = match self.fetch()? {
            FLOAT_64 => self.fetch_f64()?,
            FLOAT_32 => self.fetch_f32()? as f64,
            NIL => f64::NAN,
            n@(MIN_POSFIXINT..=MAX_POSFIXINT|NEGFIXINT..=0xff) => {
                (n as i8) as f64
            }
            UINT_8  => self.fetch_u8()?  as f64,
            UINT_16 => self.fetch_u16()? as f64,
            UINT_32 => self.fetch_u32()? as f64,
            UINT_64 => self.fetch_u64()? as f64,
            INT_8   => self.fetch_i8()?  as f64,
            INT_16  => self.fetch_i16()? as f64,
            INT_32  => self.fetch_i32()? as f64,
            INT_64  => self.fetch_i64()? as f64,
            _ => return Err(Error::ExpectedNumber)
        };
        visitor.visit_f64(f)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        let s = self.parse_str()?;
        let ch = char::from_str(s).map_err(|_| Error::InvalidLength)?;
        visitor.visit_char(ch)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_borrowed_str(self.parse_str()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_borrowed_bytes(self.parse_bytes()?)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        match self.peek()? {
            NIL => {
                self.eat_some(1);
                visitor.visit_none()
            }
            _ => visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        match self.fetch()? {
            NIL => visitor.visit_unit(),
            _ => Err(Error::ExpectedNil)
        }
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value>
        where V: Visitor<'de>
    {
        self.deserialize_unit(visitor)
    }

    // As is done here, serializers are encouraged to treat newtype structs as
    // insignificant wrappers around the data they contain. That means not
    // parsing anything other than the contained value.
    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value>
        where V: Visitor<'de>
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        let len: usize = match self.fetch()? {
            c@(FIXARRAY..=FIXARRAY_MAX) => (c as usize) & MAX_FIXARRAY_SIZE,
            ARRAY_16 => self.fetch_u16()?.into(),
            ARRAY_32 => self.fetch_u32()?.try_into()?,
            _ => return Err(Error::ExpectedArray)
        };
        let mut access = CountingAccess::new(self, len);
        let value = visitor.visit_seq(&mut access)?;
        if access.count.is_some() {
            return Err(Error::TrailingElements)
        }
        Ok(value)
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value>
        where V: Visitor<'de>
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        let len: usize = match self.fetch()? {
            c@(FIXMAP..=FIXMAP_MAX) => (c as usize) & MAX_FIXMAP_SIZE,
            MAP_16 => self.fetch_u16()?.into(),
            MAP_32 => self.fetch_u32()?.try_into()?,
            _ => return Err(Error::ExpectedMap)
        };
        let mut access = CountingAccess::new(self, len);
        let value = visitor.visit_map(&mut access)?;
        if access.count.is_some() {
            return Err(Error::TrailingElements)
        }
        Ok(value)
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
        where V: Visitor<'de>
    {
        let (map, len): (bool, usize) = match self.fetch()? {
            c@(FIXMAP..=FIXMAP_MAX) => (true, (c as usize) & MAX_FIXMAP_SIZE),
            MAP_16 => (true, self.fetch_u16()?.into()),
            MAP_32 => (true, self.fetch_u32()?.try_into()?),
            c@(FIXARRAY..=FIXARRAY_MAX) => (false, (c as usize) & MAX_FIXARRAY_SIZE),
            ARRAY_16 => (false, self.fetch_u16()?.into()),
            ARRAY_32 => (false, self.fetch_u32()?.try_into()?),
            _ => return Err(Error::ExpectedStruct)
        };
        let mut access = CountingAccess::new(self, len);
        let value = if map {
            visitor.visit_map(&mut access)?
        }
        else {
            visitor.visit_seq(&mut access)?
        };
        if access.count.is_some() {
            return Err(Error::TrailingElements)
        }
        Ok(value)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
        where V: Visitor<'de>
    {
        const FIXMAP_1: u8 = FIXMAP|1;
        match self.peek()? {
            FIXMAP_1 => {
                self.eat_some(1);
                visitor.visit_enum(VariantAccess { de: self })
            }
            _ => visitor.visit_enum(UnitVariantAccess { de: self })
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        match self.peek()? {
            MIN_POSFIXINT..=MAX_POSFIXINT|
            UINT_8|
            UINT_16|
            UINT_32 => self.deserialize_u32(visitor),
            FIXSTR..=FIXSTR_MAX|
            STR_8|
            STR_16|
            STR_32  => self.deserialize_str(visitor),
            _ => Err(Error::ExpectedIdentifier)
        }
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
        where V: Visitor<'de>
    {
        self.eat_message()?;
        visitor.visit_unit()
    }
}

struct CountingAccess<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    count: Option<NonZeroUsize>,
}

impl<'a, 'de> CountingAccess<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>, count: usize) -> Self {
        CountingAccess {
            de,
            count: NonZeroUsize::new(count),
        }
    }
}

impl<'de, 'a> SeqAccess<'de> for CountingAccess<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
        where T: DeserializeSeed<'de>
    {
        if let Some(len) = self.count {
            self.count = NonZeroUsize::new(len.get() - 1);
            return seed.deserialize(&mut *self.de).map(Some)
        }
        Ok(None)
    }

    fn size_hint(&self) -> Option<usize> {
        self.count.map(NonZeroUsize::get).or(Some(0))
    }
}

impl<'a, 'de> MapAccess<'de> for CountingAccess<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
        where K: DeserializeSeed<'de>
    {
        if let Some(len) = self.count {
            self.count = NonZeroUsize::new(len.get() - 1);
            return seed.deserialize(&mut *self.de).map(Some)
        }
        Ok(None)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
        where V: DeserializeSeed<'de>
    {
        seed.deserialize(&mut *self.de)
    }

    fn size_hint(&self) -> Option<usize> {
        self.count.map(NonZeroUsize::get).or(Some(0))
    }
}

struct UnitVariantAccess<'a, 'de> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> de::EnumAccess<'de> for UnitVariantAccess<'a, 'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self)>
        where V: de::DeserializeSeed<'de>
    {
        let variant = seed.deserialize(&mut *self.de)?;
        Ok((variant, self))
    }
}

impl<'a, 'de> de::VariantAccess<'de> for UnitVariantAccess<'a, 'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value>
        where T: de::DeserializeSeed<'de>
    {
        Err(Error::InvalidType)
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value>
        where V: de::Visitor<'de>
    {
        Err(Error::InvalidType)
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], _visitor: V) -> Result<V::Value>
        where V: de::Visitor<'de>
    {
        Err(Error::InvalidType)
    }
}

struct VariantAccess<'a, 'de> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> de::EnumAccess<'de> for VariantAccess<'a, 'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self)>
        where V: de::DeserializeSeed<'de>
    {
        let variant = seed.deserialize(&mut *self.de)?;
        Ok((variant, self))
    }
}

impl<'a, 'de> de::VariantAccess<'de> for VariantAccess<'a, 'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Err(Error::InvalidType)
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
        where T: de::DeserializeSeed<'de>
    {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
        where V: de::Visitor<'de>
    {
        de::Deserializer::deserialize_seq(self.de, visitor)
    }

    fn struct_variant<V>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
        where V: de::Visitor<'de>
    {
        de::Deserializer::deserialize_struct(self.de, "", fields, visitor)
    }
}


#[cfg(test)]
mod tests {
    #[cfg(feature = "std")]
    use std::{vec, vec::Vec, collections::BTreeMap, format};
    #[cfg(all(feature = "alloc",not(feature = "std")))]
    use alloc::{vec, vec::Vec, collections::BTreeMap, format};
    use serde::Deserialize;
    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Unit;
    #[derive(Debug, Deserialize, PartialEq)]
    struct Test {
        compact: bool,
        schema: u32,
        unit: Unit
    }

    #[test]
    fn test_deserializer() {
        let input = [0xC0];
        let mut de = Deserializer::from_slice(&input);
        assert_eq!(serde::de::Deserializer::is_human_readable(&(&mut de)), false);
        assert_eq!(de.input_ref().unwrap(), &[0xC0]);
        assert_eq!(de.remaining_len(), 1);
        assert_eq!(de.fetch().unwrap(), 0xC0);
        assert_eq!(de.input_ref().unwrap(), &[]);
        assert_eq!(de.remaining_len(), 0);
        assert_eq!(de.split_input(2), Err(Error::UnexpectedEof));
        de.eat_some(1);
        assert_eq!(de.peek(), Err(Error::UnexpectedEof));
        assert_eq!(de.fetch(), Err(Error::UnexpectedEof));
        assert_eq!(de.remaining_len(), 0);
        assert_eq!(de.input_ref(), Err(Error::UnexpectedEof));
        assert_eq!(de.split_input(1), Err(Error::UnexpectedEof));
    }

    #[test]
    fn test_de_msgpack() {
        let test = Test {
            compact: true,
            schema: 0,
            unit: Unit
        };
        assert_eq!(
            from_slice(b"\x83\xA7compact\xC3\xA6schema\x00\xA4unit\xC0"),
            Ok((test, 24))
        );
        assert_eq!(
            from_slice::<()>(b"\xC1"),
            Err(Error::ExpectedNil)
        );
        assert_eq!(
            Deserializer::from_slice(b"\xC1").eat_message(),
            Err(Error::ReservedCode)
        );
    }

    #[test]
    fn test_de_array() {
        assert_eq!(from_slice::<[i32; 0]>(&[0x90]), Ok(([], 1)));
        assert_eq!(from_slice(&[0x93, 0, 1, 2]), Ok(([0, 1, 2], 4)));
        assert_eq!(from_slice(&[0x9F, 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15]),
                              Ok(([1,2,3,4,5,6,7,8,9,10,11,12,13,14,15], 16)));
        assert_eq!(from_slice(&[0xDC, 0, 3, 0, 1, 2]), Ok(([0, 1, 2], 6)));
        assert_eq!(from_slice(&[0xDD, 0, 0, 0, 3, 0, 1, 2]), Ok(([0, 1, 2], 8)));

        #[cfg(any(feature = "std", feature = "alloc"))]
        {
            let mut vec = vec![0xDC, 0xFF, 0xFF];
            for _ in 0..65535 {
                vec.push(0xC3);
            }
            let (res, len) = from_slice::<Vec<bool>>(&vec).unwrap();
            assert_eq!(len, 65535+3);
            assert_eq!(res.len(), 65535);
            for i in 0..65535 {
                assert_eq!(res[i], true);
            }

            let mut vec = vec![0xDD, 0x00, 0x01, 0x00, 0x00];
            for _ in 0..65536 {
                vec.push(0xC2);
            }
            let (res, len) = from_slice::<Vec<bool>>(&vec).unwrap();
            assert_eq!(len, 65536+5);
            assert_eq!(res.len(), 65536);
            for i in 0..65536 {
                assert_eq!(res[i], false);
            }
        }

        // error
        assert_eq!(from_slice::<[i32; 2]>(&[0x80]), Err(Error::ExpectedArray));
        assert_eq!(from_slice::<[i32; 2]>(&[]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<[i32; 2]>(&[0x91]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<[i32; 2]>(&[0x92,0x00]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<[i32; 2]>(&[0x92,0xC0]), Err(Error::ExpectedInteger));
        assert_eq!(from_slice::<[i32; 2]>(&[0xDC]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<[i32; 2]>(&[0xDC,0x00,0x01]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<[i32; 2]>(&[0xDD]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<[i32; 2]>(&[0xDD,0x00,0x00,0x00,0x01]), Err(Error::UnexpectedEof));
    }

    #[test]
    fn test_de_bool() {
        assert_eq!(from_slice(&[0xC2]), Ok((false, 1)));
        assert_eq!(from_slice(&[0xC3]), Ok((true, 1)));
        // error
        assert_eq!(from_slice::<bool>(&[]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<bool>(&[0xC1]), Err(Error::InvalidType));
        assert_eq!(from_slice::<bool>(&[0xC0]), Err(Error::InvalidType));
    }

    #[test]
    fn test_de_floating_point() {
        assert_eq!(from_slice(&[1]), Ok((1.0f64, 1)));
        assert_eq!(from_slice(&[-1i8 as _]), Ok((-1.0f64, 1)));
        assert_eq!(from_slice(&[0xCC, 1]), Ok((1.0f64, 2)));
        assert_eq!(from_slice(&[0xCD, 0, 1]), Ok((1.0f64, 3)));
        assert_eq!(from_slice(&[0xCE, 0, 0, 0, 1]), Ok((1.0f64, 5)));
        assert_eq!(from_slice(&[0xCF, 0, 0, 0, 0, 0, 0, 0, 1]), Ok((1.0f64, 9)));
        assert_eq!(from_slice(&[0xD0, 0xff]), Ok((-1.0f64, 2)));
        assert_eq!(from_slice(&[0xD1, 0xff, 0xff]), Ok((-1.0f64, 3)));
        assert_eq!(from_slice(&[0xD2, 0xff, 0xff, 0xff, 0xff]), Ok((-1.0f64, 5)));
        assert_eq!(from_slice(&[0xD3, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]), Ok((-1.0f64, 9)));

        assert_eq!(from_slice(&[5]), Ok((5.0f32, 1)));
        assert_eq!(from_slice(&[0xCC, 1]), Ok((1.0f32, 2)));
        assert_eq!(from_slice(&[0xCD, 0, 1]), Ok((1.0f32, 3)));
        assert_eq!(from_slice(&[0xCE, 0, 0, 0, 1]), Ok((1.0f32, 5)));
        assert_eq!(from_slice(&[0xCF, 0, 0, 0, 0, 0, 0, 0, 1]), Ok((1.0f32, 9)));
        assert_eq!(from_slice(&[0xD0, 0xff]), Ok((-1.0f32, 2)));
        assert_eq!(from_slice(&[0xD1, 0xff, 0xff]), Ok((-1.0f32, 3)));
        assert_eq!(from_slice(&[0xD2, 0xff, 0xff, 0xff, 0xff]), Ok((-1.0f32, 5)));
        assert_eq!(from_slice(&[0xD3, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]), Ok((-1.0f32, 9)));

        let mut input = [0xCA, 0, 0, 0, 0];
        input[1..].copy_from_slice(&(-2.5f32).to_be_bytes());
        assert_eq!(from_slice(&input), Ok((-2.5, 5)));
        assert_eq!(from_slice(&input), Ok((-2.5f32, 5)));
        let mut input = [0xCB, 0, 0, 0, 0, 0, 0, 0, 0];
        input[1..].copy_from_slice(&(-999.9f64).to_be_bytes());
        assert_eq!(from_slice(&input), Ok((-999.9, 9)));
        assert_eq!(from_slice(&input), Ok((-999.9f32, 9)));
        let (f, len) = from_slice::<f32>(&[0xC0]).unwrap();
        assert_eq!(len, 1);
        assert!(f.is_nan());
        let (f, len) = from_slice::<f64>(&[0xC0]).unwrap();
        assert_eq!(len, 1);
        assert!(f.is_nan());
        // error
        assert_eq!(from_slice::<f32>(&[0xc1]), Err(Error::ExpectedNumber));
        assert_eq!(from_slice::<f64>(&[0x90]), Err(Error::ExpectedNumber));
        assert_eq!(from_slice::<f32>(&[]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<f64>(&[]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<f32>(&[0xCA, 0]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<f64>(&[0xCB, 0]), Err(Error::UnexpectedEof));
        for code in [0xCA, 0xCB, 
                     0xCC, 0xCD, 0xCE, 0xCF,
                     0xD0, 0xD1, 0xD2, 0xD3]
        {
            assert_eq!(from_slice::<f32>(&[code]), Err(Error::UnexpectedEof));
            assert_eq!(from_slice::<f64>(&[code]), Err(Error::UnexpectedEof));
        }
    }

    #[test]
    fn test_de_integer() {
        macro_rules! test_integer {
            ($($ty:ty),*) => {$(
                assert_eq!(from_slice::<$ty>(&[1]), Ok((1, 1)));
                assert_eq!(from_slice::<$ty>(&[-1i8 as _]), Ok((-1, 1)));
                assert_eq!(from_slice::<$ty>(&[0xCC, 1]), Ok((1, 2)));
                assert_eq!(from_slice::<$ty>(&[0xCD, 0, 1]), Ok((1, 3)));
                assert_eq!(from_slice::<$ty>(&[0xCE, 0, 0, 0, 1]), Ok((1, 5)));
                assert_eq!(from_slice::<$ty>(&[0xCF, 0, 0, 0, 0, 0, 0, 0, 1]), Ok((1, 9)));
                assert_eq!(from_slice::<$ty>(&[0xD0, 0xff]), Ok((-1, 2)));
                assert_eq!(from_slice::<$ty>(&[0xD1, 0xff, 0xff]), Ok((-1, 3)));
                assert_eq!(from_slice::<$ty>(&[0xD2, 0xff, 0xff, 0xff, 0xff]), Ok((-1, 5)));
                assert_eq!(from_slice::<$ty>(&[0xD3, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]), Ok((-1, 9)));

            )*};
        }
        macro_rules! test_unsigned {
            ($($ty:ty),*) => {$(
                assert_eq!(from_slice::<$ty>(&[1]), Ok((1, 1)));
                assert_eq!(from_slice::<$ty>(&[-1i8 as _]), Err(Error::InvalidInteger));
                assert_eq!(from_slice::<$ty>(&[0xCC, 1]), Ok((1, 2)));
                assert_eq!(from_slice::<$ty>(&[0xCD, 0, 1]), Ok((1, 3)));
                assert_eq!(from_slice::<$ty>(&[0xCE, 0, 0, 0, 1]), Ok((1, 5)));
                assert_eq!(from_slice::<$ty>(&[0xCF, 0, 0, 0, 0, 0, 0, 0, 1]), Ok((1, 9)));
                assert_eq!(from_slice::<$ty>(&[0xD0, 1]), Ok((1, 2)));
                assert_eq!(from_slice::<$ty>(&[0xD0, 0xff]), Err(Error::InvalidInteger));
                assert_eq!(from_slice::<$ty>(&[0xD1, 0, 1]), Ok((1, 3)));
                assert_eq!(from_slice::<$ty>(&[0xD1, 0xff, 0xff]), Err(Error::InvalidInteger));
                assert_eq!(from_slice::<$ty>(&[0xD2, 0, 0, 0, 1]), Ok((1, 5)));
                assert_eq!(from_slice::<$ty>(&[0xD2, 0xff, 0xff, 0xff, 0xff]), Err(Error::InvalidInteger));
                assert_eq!(from_slice::<$ty>(&[0xD3, 0, 0, 0, 0, 0, 0, 0, 1]), Ok((1, 9)));
                assert_eq!(from_slice::<$ty>(&[0xD3, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]), Err(Error::InvalidInteger));
            )*};
        }
        macro_rules! test_int_err {
            ($($ty:ty),*) => {$(
                assert_eq!(from_slice::<$ty>(&[0xC0]), Err(Error::ExpectedInteger));
                assert_eq!(from_slice::<$ty>(&[0xCC]), Err(Error::UnexpectedEof));
                assert_eq!(from_slice::<$ty>(&[0xCD]), Err(Error::UnexpectedEof));
                assert_eq!(from_slice::<$ty>(&[0xCE]), Err(Error::UnexpectedEof));
                assert_eq!(from_slice::<$ty>(&[0xCF]), Err(Error::UnexpectedEof));
                assert_eq!(from_slice::<$ty>(&[0xCD, 0]), Err(Error::UnexpectedEof));
                assert_eq!(from_slice::<$ty>(&[0xCE, 0]), Err(Error::UnexpectedEof));
                assert_eq!(from_slice::<$ty>(&[0xCF, 0]), Err(Error::UnexpectedEof));
                assert_eq!(from_slice::<$ty>(&[0xD0]), Err(Error::UnexpectedEof));
                assert_eq!(from_slice::<$ty>(&[0xD1]), Err(Error::UnexpectedEof));
                assert_eq!(from_slice::<$ty>(&[0xD2]), Err(Error::UnexpectedEof));
                assert_eq!(from_slice::<$ty>(&[0xD3]), Err(Error::UnexpectedEof));
                assert_eq!(from_slice::<$ty>(&[0xD1, 0]), Err(Error::UnexpectedEof));
                assert_eq!(from_slice::<$ty>(&[0xD2, 0]), Err(Error::UnexpectedEof));
                assert_eq!(from_slice::<$ty>(&[0xD3, 0]), Err(Error::UnexpectedEof));
            )*};
        }
        test_integer!(i8,i16,i32,i64);
        test_unsigned!(u8,u16,u32,u64);
        test_int_err!(i8,i16,i32,i64, u8,u16,u32,u64);
        assert_eq!(from_slice::<i8>(&[0xCC, 0x80]), Err(Error::InvalidInteger));
        assert_eq!(from_slice::<i16>(&[0xCD, 0x80, 0x00]), Err(Error::InvalidInteger));
        assert_eq!(from_slice::<i32>(&[0xCE, 0x80, 0x00, 0x00, 0x00]), Err(Error::InvalidInteger));
        assert_eq!(from_slice::<i64>(&[0xCF, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), Err(Error::InvalidInteger));
    }

    #[test]
    fn test_de_char() {
        assert_eq!(from_slice::<char>(&[0xA4,0xf0,0x9f,0x91,0x8f]), Ok(('👏', 5)));
        assert_eq!(from_slice::<char>(&[0xD9,4,0xf0,0x9f,0x91,0x8f]), Ok(('👏', 6)));
        assert_eq!(from_slice::<char>(&[0xDA,0,4,0xf0,0x9f,0x91,0x8f]), Ok(('👏', 7)));
        assert_eq!(from_slice::<char>(&[0xDB,0,0,0,4,0xf0,0x9f,0x91,0x8f]), Ok(('👏', 9)));
        assert_eq!(from_slice::<char>(b""), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<char>(b"\xC0"), Err(Error::ExpectedString));
        assert_eq!(from_slice::<char>(b"\xA0"), Err(Error::InvalidLength));
        assert_eq!(from_slice::<char>(b"\xA2ab"), Err(Error::InvalidLength));
        assert_eq!(from_slice::<char>(b"\xA1"), Err(Error::UnexpectedEof));
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn test_de_string() {
        assert_eq!(from_slice::<String>(&[0xA0]), Ok(("".to_string(), 1)));
        assert_eq!(from_slice::<String>(&[0xD9,0]), Ok(("".to_string(), 2)));
        assert_eq!(from_slice::<String>(&[0xDA,0,0]), Ok(("".to_string(), 3)));
        assert_eq!(from_slice::<String>(&[0xDB,0,0,0,0]), Ok(("".to_string(), 5)));
        assert_eq!(from_slice::<String>(&[0xA1]), Err(Error::UnexpectedEof));
    }

    #[test]
    fn test_de_str() {
        assert_eq!(from_slice(&[0xA0]), Ok(("", 1)));
        assert_eq!(from_slice(&[0xD9,0]), Ok(("", 2)));
        assert_eq!(from_slice(&[0xDA,0,0]), Ok(("", 3)));
        assert_eq!(from_slice(&[0xDB,0,0,0,0]), Ok(("", 5)));
        assert_eq!(from_slice(&[0xA4,0xf0,0x9f,0x91,0x8f]), Ok(("👏", 5)));
        assert_eq!(from_slice(&[0xD9,4,0xf0,0x9f,0x91,0x8f]), Ok(("👏", 6)));
        assert_eq!(from_slice(&[0xDA,0,4,0xf0,0x9f,0x91,0x8f]), Ok(("👏", 7)));
        assert_eq!(from_slice(&[0xDB,0,0,0,4,0xf0,0x9f,0x91,0x8f]), Ok(("👏", 9)));
        assert_eq!(from_slice(b"\xBF01234567890ABCDEFGHIJKLMNOPQRST"),
                   Ok(("01234567890ABCDEFGHIJKLMNOPQRST", 32)));
        let text = "O, mógłże sęp chlań wyjść furtką bździn";
        let mut input = [0u8;50];
        input[..2].copy_from_slice(&[0xd9, text.len() as u8]);
        input[2..].copy_from_slice(text.as_bytes());
        assert_eq!(from_slice(&input), Ok((text, 50)));
        // error
        assert_eq!(from_slice::<&str>(&[0xC4]), Err(Error::ExpectedString));
        assert_eq!(from_slice::<&str>(b"\xA2\xff\xfe"), Err(Error::InvalidUnicodeCodePoint));
        assert_eq!(from_slice::<&str>(&[]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<&str>(&[0xA1]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<&str>(&[0xA2, 0]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<&str>(&[0xD9]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<&str>(&[0xD9, 1]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<&str>(&[0xDA, 0]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<&str>(&[0xDA, 0, 1]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<&str>(&[0xDB, 0, 0, 0]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<&str>(&[0xDB, 0, 0, 0, 1]), Err(Error::UnexpectedEof));
    }

    #[test]
    fn test_de_bytes() {
        assert_eq!(from_slice::<&[u8]>(&[0xC4,0]), Ok((&[][..], 2)));
        assert_eq!(from_slice::<&[u8]>(&[0xC5,0,0]), Ok((&[][..], 3)));
        assert_eq!(from_slice::<&[u8]>(&[0xC6,0,0,0,0]), Ok((&[][..], 5)));
        assert_eq!(from_slice::<&[u8]>(&[0xC4,1,0xff]), Ok((&[0xff][..], 3)));
        assert_eq!(from_slice::<&[u8]>(&[0xC5,0,1,0xff]), Ok((&[0xff][..], 4)));
        assert_eq!(from_slice::<&[u8]>(&[0xC6,0,0,0,1,0xff]), Ok((&[0xff][..], 6)));
        // error
        assert_eq!(from_slice::<&[u8]>(&[0xA0]), Err(Error::ExpectedBin));
        assert_eq!(from_slice::<&[u8]>(&[]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<&[u8]>(&[0xC4]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<&[u8]>(&[0xC4, 1]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<&[u8]>(&[0xC5, 0]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<&[u8]>(&[0xC5, 0, 1]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<&[u8]>(&[0xC6, 0, 0, 0]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<&[u8]>(&[0xC6, 0, 0, 0, 1]), Err(Error::UnexpectedEof));
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn test_de_bytes_own() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Bytes(#[serde(with = "serde_bytes")] Vec<u8>);
        assert_eq!(from_slice::<Bytes>(&[0xC4,0]), Ok((Bytes(Vec::new()), 2)));
        assert_eq!(from_slice::<Bytes>(&[0xC5,0,0]), Ok((Bytes(Vec::new()), 3)));
        assert_eq!(from_slice::<Bytes>(&[0xC6,0,0,0,0]), Ok((Bytes(Vec::new()), 5)));
        assert_eq!(from_slice::<Bytes>(&[0xC4,1,0xff]), Ok((Bytes(vec![0xff]), 3)));
        assert_eq!(from_slice::<Bytes>(&[0xC5,0,1,0xff]), Ok((Bytes(vec![0xff]), 4)));
        assert_eq!(from_slice::<Bytes>(&[0xC6,0,0,0,1,0xff]), Ok((Bytes(vec![0xff]), 6)));
        // error
        assert_eq!(from_slice::<Bytes>(&[0xA0]), Err(Error::ExpectedBin));
        assert_eq!(from_slice::<Bytes>(&[]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<Bytes>(&[0xC4]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<Bytes>(&[0xC4, 1]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<Bytes>(&[0xC5, 0]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<Bytes>(&[0xC5, 0, 1]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<Bytes>(&[0xC6, 0, 0, 0]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<Bytes>(&[0xC6, 0, 0, 0, 1]), Err(Error::UnexpectedEof));
    }

    #[derive(Debug, Deserialize, PartialEq)]
    enum Type {
        #[serde(rename = "boolean")]
        Boolean,
        #[serde(rename = "number")]
        Number,
        #[serde(rename = "thing")]
        Thing,
    }

    #[test]
    fn test_de_enum_clike() {
        assert_eq!(from_slice(b"\xA7boolean"), Ok((Type::Boolean, 8)));
        assert_eq!(from_slice(b"\xA6number"), Ok((Type::Number, 7)));
        assert_eq!(from_slice(b"\xA5thing"), Ok((Type::Thing, 6)));

        assert_eq!(from_slice(b"\x00"), Ok((Type::Boolean, 1)));
        assert_eq!(from_slice(b"\x01"), Ok((Type::Number, 1)));
        assert_eq!(from_slice(b"\x02"), Ok((Type::Thing, 1)));
        // error
        #[cfg(any(feature = "std", feature = "alloc"))]
        assert_eq!(from_slice::<Type>(b"\xA0"), Err(Error::DeserializeError(
            r#"unknown variant ``, expected one of `boolean`, `number`, `thing`"#.into())));
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        assert_eq!(from_slice::<Type>(b"\xA0"), Err(Error::DeserializeError));

        #[cfg(any(feature = "std", feature = "alloc"))]
        assert_eq!(from_slice::<Type>(b"\xA3xyz"), Err(Error::DeserializeError(
            r#"unknown variant `xyz`, expected one of `boolean`, `number`, `thing`"#.into())));
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        assert_eq!(from_slice::<Type>(b"\xA3xyz"), Err(Error::DeserializeError));

        #[cfg(any(feature = "std", feature = "alloc"))]
        assert_eq!(from_slice::<Type>(b"\x03"), Err(Error::DeserializeError(
            r#"invalid value: integer `3`, expected variant index 0 <= i < 3"#.into())));
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        assert_eq!(from_slice::<Type>(b"\x03"), Err(Error::DeserializeError));
        assert_eq!(from_slice::<Type>(&[0xC0]), Err(Error::ExpectedIdentifier));
        assert_eq!(from_slice::<Type>(&[0x80]), Err(Error::ExpectedIdentifier));
        assert_eq!(from_slice::<Type>(&[0x90]), Err(Error::ExpectedIdentifier));
        assert_eq!(from_slice::<Type>(b""), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<Type>(b"\x81\xA7boolean\xC0"), Err(Error::InvalidType));
        assert_eq!(from_slice::<Type>(b"\x81\xA7boolean\x90"), Err(Error::InvalidType));
        assert_eq!(from_slice::<Type>(b"\x81\xA7boolean\x80"), Err(Error::InvalidType));
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn test_de_map() {
        let (map, len) = from_slice::<BTreeMap<i32,&str>>(
            b"\x83\xff\xA1A\xfe\xA3wee\xD1\x01\xA4\xD9\x24Waltz, bad nymph, for quick jigs vex").unwrap();
        assert_eq!(len, 50);
        assert_eq!(map.len(), 3);
        assert_eq!(map[&-1], "A");
        assert_eq!(map[&-2], "wee");
        assert_eq!(map[&420], "Waltz, bad nymph, for quick jigs vex");

        let (map, len) = from_slice::<BTreeMap<i32,bool>>(&[0x80]).unwrap();
        assert_eq!(len, 1);
        assert_eq!(map.len(), 0);

        let (map, len) = from_slice::<BTreeMap<i32,bool>>(
            b"\x8F\x01\xC3\x02\xC3\x03\xC3\x04\xC3\x05\xC3\x06\xC3\x07\xC3\x08\xC3\x09\xC3\x0A\xC3\x0B\xC3\x0C\xC3\x0D\xC3\x0E\xC3\x0F\xC3").unwrap();
        assert_eq!(len, 31);
        assert_eq!(map.len(), 15);
        for i in 1..=15 {
            assert_eq!(map[&i], true);
        }

        let mut vec = vec![0xDE, 0xFF, 0xFF];
        vec.reserve(65536*2);
        for i in 1..=65535u16 {
            if i < 128 {
                vec.push(i as u8);
            }
            else if i < 256 {
                vec.push(0xCC);
                vec.push(i as u8);
            }
            else {
                vec.push(0xCD);
                vec.extend_from_slice(&i.to_be_bytes());
            }
            vec.push(0xC3);
        }
        let (map, len) = from_slice::<BTreeMap<u32,bool>>(vec.as_slice()).unwrap();
        assert_eq!(len, vec.len());
        assert_eq!(map.len(), 65535);
        for i in 1..=65535 {
            assert!(map[&i]);
        }

        let mut vec = vec![0xDF,0x00,0x01,0x00,0x00];
        vec.reserve(65536*2);
        for i in 1..=65536u32 {
            if i < 128 {
                vec.push(i as u8);
            }
            else if i < 256 {
                vec.push(0xCC);
                vec.push(i as u8);
            }
            else if i < 65536 {
                vec.push(0xCD);
                vec.extend_from_slice(&(i as u16).to_be_bytes());
            }
            else {
                vec.push(0xCE);
                vec.extend_from_slice(&i.to_be_bytes());
            }
            vec.push(0xC3);
        }
        let (map, len) = from_slice::<BTreeMap<u32,bool>>(vec.as_slice()).unwrap();
        assert_eq!(len, vec.len());
        assert_eq!(map.len(), 65536);
        for i in 1..=65536 {
            assert!(map[&i]);
        }

        // error
        assert_eq!(from_slice::<BTreeMap<i32,bool>>(&[0x90]), Err(Error::ExpectedMap));
        assert_eq!(from_slice::<BTreeMap<i32,bool>>(&[]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<BTreeMap<i32,bool>>(&[0x81]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<BTreeMap<i32,bool>>(&[0x81,0x00]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<BTreeMap<i32,bool>>(&[0x82,0x00,0xC2]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<BTreeMap<i32,bool>>(&[0xDE]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<BTreeMap<i32,bool>>(&[0xDE,0x00,0x01]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<BTreeMap<i32,bool>>(&[0xDF]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<BTreeMap<i32,bool>>(&[0xDF,0x00,0x00,0x00,0x01]), Err(Error::UnexpectedEof));
    }

    #[test]
    fn test_de_map_err() {
        use core::marker::PhantomData;
        use serde::de::Deserializer;
        #[derive(Debug, PartialEq)]
        struct PhonyMap(Option<(i32,i32)>);
        struct PhonyMapVisitor(PhantomData<PhonyMap>);
        impl<'de> Visitor<'de> for PhonyMapVisitor {
            type Value = PhonyMap;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut access: M) -> core::result::Result<Self::Value, M::Error> {
                if let Some((k, v)) = access.next_entry()? {
                    return Ok(PhonyMap(Some((k,v))))
                }
                Ok(PhonyMap(None))
            }
        }
        impl<'de> Deserialize<'de> for PhonyMap {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> core::result::Result<Self, D::Error> {
                deserializer.deserialize_any(PhonyMapVisitor(PhantomData))
            }
        }
        assert_eq!(
            from_slice::<PhonyMap>(b"\x80"),
            Ok((PhonyMap(None), 1)));
        assert_eq!(
            from_slice::<PhonyMap>(b"\x81\x00\x01"),
            Ok((PhonyMap(Some((0,1))), 3)));
        assert_eq!(from_slice::<PhonyMap>(b""), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<PhonyMap>(b"\x81"), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<PhonyMap>(b"\x81\x00"), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<PhonyMap>(b"\x82\x00\x01"), Err(Error::TrailingElements));
        assert_eq!(from_slice::<PhonyMap>(b"\x82\x00\x01"), Err(Error::TrailingElements));
        assert!(from_slice::<PhonyMap>(b"\x90").is_err());
    }

    #[test]
    fn test_de_struct() {
        #[derive(Default, Debug, Deserialize, PartialEq)]
        #[serde(default)]
        struct Test<'a> {
            foo: i8,
            bar: &'a str
        }
        assert_eq!(
            from_slice(&[0x82,
                0xA3, b'f', b'o', b'o', 0xff,
                0xA3, b'b', b'a', b'r', 0xA3, b'b', b'a', b'z']),
            Ok((Test { foo: -1, bar: "baz" }, 14))
        );
        assert_eq!(
            from_slice(&[0xDE,0x00,0x02,
                0xD9,0x03, b'f', b'o', b'o', 0xff,
                0xDA,0x00,0x03, b'b', b'a', b'r', 0xDB, 0x00, 0x00, 0x00, 0x03, b'b', b'a', b'z']),
            Ok((Test { foo: -1, bar: "baz" }, 23))
        );
        assert_eq!(
            from_slice(&[0xDF,0x00,0x00,0x00,0x02,
                0xD9,0x03, b'f', b'o', b'o', 0xff,
                0xDA,0x00,0x03, b'b', b'a', b'r', 0xDB, 0x00, 0x00, 0x00, 0x03, b'b', b'a', b'z']),
            Ok((Test { foo: -1, bar: "baz" }, 25))
        );

        assert_eq!(
            from_slice(&[0x82,
                0x00, 0xff,
                0x01, 0xA3, b'b', b'a', b'z']),
            Ok((Test { foo: -1, bar: "baz" }, 8))
        );

        assert_eq!(
            from_slice(&[0x92, 0xff, 0xA3, b'b', b'a', b'z']),
            Ok((Test { foo: -1, bar: "baz" }, 6))
        );
        assert_eq!(
            from_slice(&[0xDC,0x00,0x02, 0xff, 0xD9,0x03, b'b', b'a', b'z']),
            Ok((Test { foo: -1, bar: "baz" }, 9))
        );
        assert_eq!(
            from_slice(&[0xDD,0x00,0x00,0x00,0x02, 0xff, 0xDB, 0x00, 0x00, 0x00, 0x03, b'b', b'a', b'z']),
            Ok((Test { foo: -1, bar: "baz" }, 14))
        );

        // error
        assert_eq!(
            from_slice::<Test>(&[0x93, 0xff, 0xA3, b'b', b'a', b'z', 0xC0]),
                Err(Error::TrailingElements));

        #[cfg(any(feature = "std", feature = "alloc"))]
        assert_eq!(
            from_slice::<Test>(&[0x84,
                0x00, 0xff,
                0x01, 0xA3, b'b', b'a', b'z',
                0x02, 0xC0,
                0xA3, b'f', b'o', b'o', 0x01]),
            Err(Error::DeserializeError("duplicate field `foo`".into()))
        );
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        assert_eq!(
            from_slice::<Test>(&[0x84,
                0x00, 0xff,
                0x01, 0xA3, b'b', b'a', b'z',
                0x02, 0xC0,
                0xA3, b'f', b'o', b'o', 0x01]),
            Err(Error::DeserializeError)
        );
        assert_eq!(from_slice::<Test>(b""), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<Test>(b"\xC0"), Err(Error::ExpectedStruct));
        assert_eq!(from_slice::<Test>(b"\x81"), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<Test>(b"\xDC"), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<Test>(b"\xDD"), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<Test>(b"\xDE"), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<Test>(b"\xDF"), Err(Error::UnexpectedEof));
    }

    #[test]
    fn test_de_struct_bool() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Led {
            led: bool,
        }

        assert_eq!(
            from_slice(b"\x81\xA3led\xC3"),
            Ok((Led { led: true }, 6)));
        assert_eq!(
            from_slice(b"\x81\x00\xC3"),
            Ok((Led { led: true }, 3)));
        assert_eq!(
            from_slice(b"\x91\xC3"),
            Ok((Led { led: true }, 2)));
        assert_eq!(
            from_slice(b"\x81\xA3led\xC2"),
            Ok((Led { led: false }, 6)));
        assert_eq!(
            from_slice(b"\x81\x00\xC2"),
            Ok((Led { led: false }, 3)));
        assert_eq!(
            from_slice(b"\x91\xC2"),
            Ok((Led { led: false }, 2)));
    }

    #[test]
    fn test_de_struct_i8() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Temperature {
            temperature: i8,
        }

        assert_eq!(
            from_slice(b"\x81\xABtemperature\xEF"),
            Ok((Temperature { temperature: -17 }, 14)));
        assert_eq!(
            from_slice(b"\x81\x00\xEF"),
            Ok((Temperature { temperature: -17 }, 3)));
        assert_eq!(
            from_slice(b"\x91\xEF"),
            Ok((Temperature { temperature: -17 }, 2)));
        // out of range
        assert_eq!(
            from_slice::<Temperature>(b"\x81\xABtemperature\xCC\x80"),
            Err(Error::InvalidInteger));
        assert_eq!(
            from_slice::<Temperature>(b"\x91\xD1\xff\x00"),
            Err(Error::InvalidInteger));
        // error
        assert_eq!(from_slice::<Temperature>(b"\x81\xABtemperature\xCA\x00\x00\x00\x00"), Err(Error::ExpectedInteger));
        assert_eq!(from_slice::<Temperature>(b"\x81\xABtemperature\xC0"), Err(Error::ExpectedInteger));
        assert_eq!(from_slice::<Temperature>(b"\x81\xABtemperature\xC2"), Err(Error::ExpectedInteger));
    }

    #[test]
    fn test_de_struct_u8() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Temperature {
            temperature: u8,
        }

        assert_eq!(
            from_slice(b"\x81\xABtemperature\x14"),
            Ok((Temperature { temperature: 20 }, 14)));
        assert_eq!(
            from_slice(b"\x81\x00\x14"),
            Ok((Temperature { temperature: 20 }, 3)));
        assert_eq!(
            from_slice(b"\x91\x14"),
            Ok((Temperature { temperature: 20 }, 2)));
        // out of range
        assert_eq!(
            from_slice::<Temperature>(b"\x81\xABtemperature\xCD\x01\x00"),
            Err(Error::InvalidInteger));
        assert_eq!(
            from_slice::<Temperature>(b"\x91\xff"),
            Err(Error::InvalidInteger));
        // error
        assert_eq!(from_slice::<Temperature>(b"\x81\xABtemperature\xCA\x00\x00\x00\x00"), Err(Error::ExpectedInteger));
        assert_eq!(from_slice::<Temperature>(b"\x81\xABtemperature\xC0"), Err(Error::ExpectedInteger));
        assert_eq!(from_slice::<Temperature>(b"\x81\xABtemperature\xC2"), Err(Error::ExpectedInteger));
    }

    #[test]
    fn test_de_struct_f32() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Temperature {
            temperature: f32,
        }

        assert_eq!(
            from_slice(b"\x81\xABtemperature\xEF"),
            Ok((Temperature { temperature: -17.0 }, 14)));
        assert_eq!(
            from_slice(b"\x81\x00\xEF"),
            Ok((Temperature { temperature: -17.0 }, 3)));
        assert_eq!(
            from_slice(b"\x91\xEF"),
            Ok((Temperature { temperature: -17.0 }, 2)));

        assert_eq!(
            from_slice(b"\x81\xABtemperature\xCA\xc1\x89\x99\x9a"),
            Ok((Temperature { temperature: -17.2 }, 18))
        );
        assert_eq!(
            from_slice(b"\x91\xCB\xBF\x61\x34\x04\xEA\x4A\x8C\x15"),
            Ok((Temperature {temperature: -2.1e-3}, 10))
        );
        // NaNs will always compare unequal.
        let (r, n): (Temperature, usize) = from_slice(b"\x81\xABtemperature\xC0").unwrap();
        assert!(r.temperature.is_nan());
        assert_eq!(n, 14);
        // error
        assert_eq!(from_slice::<Temperature>(b"\x81\xABtemperature\xC2"), Err(Error::ExpectedNumber));
    }

    #[test]
    fn test_de_struct_option() {
        #[derive(Default, Debug, Deserialize, PartialEq)]
        #[serde(default)]
        struct Property<'a> {
            description: Option<&'a str>,
            value: Option<u32>,
        }

        assert_eq!(
            from_slice(b"\x81\xABdescription\xBDAn ambient temperature sensor"),
            Ok((Property {description: Some("An ambient temperature sensor"), value: None}, 43)));
        assert_eq!(
            from_slice(b"\x81\x00\xBDAn ambient temperature sensor"),
            Ok((Property {description: Some("An ambient temperature sensor"), value: None}, 32)));
        assert_eq!(
            from_slice(b"\x91\xBDAn ambient temperature sensor"),
            Ok((Property {description: Some("An ambient temperature sensor"), value: None}, 31)));

        assert_eq!(
            from_slice(b"\x80"),
            Ok((Property { description: None, value: None }, 1)));
        assert_eq!(
            from_slice(b"\x81\xABdescription\xC0"),
            Ok((Property { description: None, value: None }, 14)));
        assert_eq!(
            from_slice(b"\x81\xA5value\xC0"),
            Ok((Property { description: None, value: None }, 8)));
        assert_eq!(
            from_slice(b"\x82\xABdescription\xC0\xA5value\xC0"),
            Ok((Property { description: None, value: None }, 21)));
        assert_eq!(
            from_slice(b"\x81\x00\xC0"),
            Ok((Property { description: None, value: None }, 3)));
        assert_eq!(
            from_slice(b"\x81\x01\xC0"),
            Ok((Property { description: None, value: None }, 3)));
        assert_eq!(
            from_slice(b"\x81\x01\x00"),
            Ok((Property { description: None, value: Some(0) }, 3)));
        assert_eq!(
            from_slice(b"\x81\x01\x7F"),
            Ok((Property { description: None, value: Some(127) }, 3)));
        assert_eq!(
            from_slice(b"\x82\x01\x7F\x00\xC0"),
            Ok((Property { description: None, value: Some(127) }, 5)));

        assert_eq!(
            from_slice(b"\x90"),
            Ok((Property { description: None, value: None }, 1)));
        assert_eq!(
            from_slice(b"\x91\xC0"),
            Ok((Property { description: None, value: None }, 2)));
        assert_eq!(
            from_slice(b"\x92\xC0\xC0"),
            Ok((Property { description: None, value: None }, 3)));
        assert_eq!(
            from_slice(b"\x91\xBDAn ambient temperature sensor"),
            Ok((Property { description: Some("An ambient temperature sensor"), value: None }, 31)));
        assert_eq!(
            from_slice(b"\x92\xBDAn ambient temperature sensor\xC0"),
            Ok((Property { description: Some("An ambient temperature sensor"), value: None }, 32)));
        assert_eq!(
            from_slice(b"\x92\xBDAn ambient temperature sensor\x00"),
            Ok((Property { description: Some("An ambient temperature sensor"), value: Some(0) }, 32)));
        assert_eq!(
            from_slice(b"\x92\xC0\x00"),
            Ok((Property { description: None, value: Some(0) }, 3)));
        assert_eq!(from_slice::<Property>(b"\x91\x00"), Err(Error::ExpectedString));
        assert_eq!(from_slice::<Property>(b"\x92\xA1x"), Err(Error::UnexpectedEof));
    }

    #[test]
    fn test_de_test_unit() {
        assert_eq!(from_slice(&[0xC0]), Ok(((), 1)));
        #[derive(Debug, Deserialize, PartialEq)]
        struct Unit;
        assert_eq!(from_slice(&[0xC0]), Ok((Unit, 1)));
    }

    #[test]
    fn test_de_newtype_struct() {
        #[derive(Deserialize, Debug, PartialEq, Clone, Copy)]
        struct A(u32);

        let a = A(54);
        assert_eq!(from_slice(&[54]), Ok((a, 1)));
        assert_eq!(from_slice(&[0xCC, 54]), Ok((a, 2)));
        assert_eq!(from_slice(&[0xCD, 0, 54]), Ok((a, 3)));
        assert_eq!(from_slice(&[0xCE, 0, 0, 0, 54]), Ok((a, 5)));
        assert_eq!(from_slice(&[0xCF, 0, 0, 0, 0, 0, 0, 0, 54]), Ok((a, 9)));
        assert_eq!(from_slice(&[0xD0, 54]), Ok((a, 2)));
        assert_eq!(from_slice(&[0xD1, 0, 54]), Ok((a, 3)));
        assert_eq!(from_slice(&[0xD2, 0, 0, 0, 54]), Ok((a, 5)));
        assert_eq!(from_slice(&[0xD3, 0, 0, 0, 0, 0, 0, 0, 54]), Ok((a, 9)));
        assert_eq!(from_slice::<A>(&[0xCA, 0x42, 0x58, 0, 0]), Err(Error::ExpectedInteger));
        assert_eq!(from_slice::<A>(&[0xCB, 0x40, 0x4B, 0, 0, 0, 0, 0, 0]), Err(Error::ExpectedInteger));

        #[derive(Deserialize, Debug, PartialEq, Clone, Copy)]
        struct B(f32);

        let b = B(54.0);
        assert_eq!(from_slice(&[54]), Ok((b, 1)));
        assert_eq!(from_slice(&[0xCC, 54]), Ok((b, 2)));
        assert_eq!(from_slice(&[0xCD, 0, 54]), Ok((b, 3)));
        assert_eq!(from_slice(&[0xCE, 0, 0, 0, 54]), Ok((b, 5)));
        assert_eq!(from_slice(&[0xCF, 0, 0, 0, 0, 0, 0, 0, 54]), Ok((b, 9)));
        assert_eq!(from_slice(&[0xD0, 54]), Ok((b, 2)));
        assert_eq!(from_slice(&[0xD1, 0, 54]), Ok((b, 3)));
        assert_eq!(from_slice(&[0xD2, 0, 0, 0, 54]), Ok((b, 5)));
        assert_eq!(from_slice(&[0xD3, 0, 0, 0, 0, 0, 0, 0, 54]), Ok((b, 9)));
        assert_eq!(from_slice(&[0xCA, 0x42, 0x58, 0, 0]), Ok((b, 5)));
        assert_eq!(from_slice(&[0xCB, 0x40, 0x4B, 0, 0, 0, 0, 0, 0]), Ok((b, 9)));
    }

    #[test]
    fn test_de_newtype_variant() {
        #[derive(Deserialize, Debug, PartialEq, Clone, Copy)]
        enum A {
            A(u32),
        }
        let a = A::A(54);
        assert_eq!(from_slice::<A>(&[0x81,0xA1,b'A',54]), Ok((a, 4)));
        assert_eq!(from_slice::<A>(&[0x81,0x00,54]), Ok((a, 3)));
        // error
        assert_eq!(from_slice::<A>(&[]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<A>(&[0x81]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<A>(&[0x00]), Err(Error::InvalidType));
        assert_eq!(from_slice::<A>(&[0xA1,b'A']), Err(Error::InvalidType));
    }

    #[test]
    fn test_de_struct_variant() {
        #[derive(Deserialize, Debug, PartialEq, Clone, Copy)]
        enum A {
            A { x: u32, y: u16 },
        }
        let a = A::A { x: 54, y: 720 };
        assert_eq!(from_slice(&[0x81,0xA1,b'A', 0x82, 0xA1,b'x',54, 0xA1,b'y',0xCD,2,208]), Ok((a, 12)));
        assert_eq!(from_slice(&[0x81,0x00, 0x82, 0xA1,b'x',54, 0xA1,b'y',0xCD,2,208]), Ok((a, 11)));
        assert_eq!(from_slice(&[0x81,0xA1,b'A', 0x82, 0x00,54, 0x01,0xCD,2,208]), Ok((a, 10)));
        assert_eq!(from_slice(&[0x81,0x00, 0x82, 0x00,54, 0x01,0xCD,2,208]), Ok((a, 9)));
        assert_eq!(from_slice(&[0x81,0xA1,b'A', 0x92 ,54, 0xCD,2,208]), Ok((a, 8)));
        assert_eq!(from_slice(&[0x81,0x00, 0x92 ,54, 0xCD,2,208]), Ok((a, 7)));
        // error
        assert_eq!(from_slice::<A>(&[]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<A>(&[0x81]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<A>(&[0x81,0xA1,b'A', 0x93 ,54, 0xCD,2,208, 0xC0]), Err(Error::TrailingElements));
        assert_eq!(from_slice::<A>(&[0x81,0x00, 0x93 ,54, 0xCD,2,208, 0xC0]), Err(Error::TrailingElements));
        assert_eq!(from_slice::<A>(&[0xA1,b'A']), Err(Error::InvalidType));
    }

    #[test]
    fn test_de_tuple_variant() {
        #[derive(Deserialize, Debug, PartialEq, Clone, Copy)]
        enum A {
            A(i32,u16),
        }
        let a = A::A(-19,10000);
        assert_eq!(from_slice(&[0x81,0xA1,b'A', 0x92 ,0xED, 0xCD,0x27,0x10]), Ok((a, 8)));
        assert_eq!(from_slice(&[0x81,0x00, 0x92 ,0xED, 0xCD,0x27,0x10]), Ok((a, 7)));
        // error
        assert_eq!(from_slice::<A>(&[]), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<A>(&[0xA1,b'A']), Err(Error::InvalidType));
        assert_eq!(from_slice::<A>(&[0x81,0xA1,b'A', 0x80]), Err(Error::ExpectedArray));
        assert_eq!(from_slice::<A>(&[0x81,0x00, 0x80]), Err(Error::ExpectedArray));
        assert_eq!(from_slice::<A>(&[0x81,0x00, 0x93 ,0xED, 0xCD,0x27,0x10, 0xC0]), Err(Error::TrailingElements));
        assert_eq!(from_slice::<A>(&[0x81,0xA1,b'A', 0x93 ,0xED, 0xCD,0x27,0x10, 0xC0]), Err(Error::TrailingElements));
    }

    #[test]
    fn test_de_struct_tuple() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Xy(u8, i8);

        assert_eq!(from_slice(&[0x92,10,20]), Ok((Xy(10, 20), 3)));
        assert_eq!(from_slice(&[0x92,0xCC,200,-20i8 as _]), Ok((Xy(200, -20), 4)));
        assert_eq!(from_slice(&[0x92,10,0xD0,-77i8 as _]), Ok((Xy(10, -77), 4)));
        assert_eq!(from_slice(&[0x92,0xCC,200,0xD0,-77i8 as _]), Ok((Xy(200, -77), 5)));

        // wrong number of args
        #[cfg(any(feature = "std", feature = "alloc"))]
        assert_eq!(
            from_slice::<Xy>(&[0x91,0x10]),
            Err(Error::DeserializeError(
                r#"invalid length 1, expected tuple struct Xy with 2 elements"#.to_string()))
        );
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        assert_eq!(
            from_slice::<Xy>(&[0x91,0x10]),
            Err(Error::DeserializeError)
        );
        assert_eq!(
            from_slice::<Xy>(&[0x93,10,20,30]),
            Err(Error::TrailingElements)
        );
    }

    #[test]
    fn test_de_struct_with_array_field() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Test {
            status: bool,
            point: [u32; 3],
        }

        assert_eq!(
            from_slice(b"\x82\xA6status\xC3\xA5point\x93\x01\x02\x03"),
            Ok((
                Test {
                    status: true,
                    point: [1, 2, 3]
                },
                19
            ))
        );
        assert_eq!(
            from_slice(b"\x82\x00\xC3\x01\x93\x01\x02\x03"),
            Ok((
                Test {
                    status: true,
                    point: [1, 2, 3]
                },
                8
            ))
        );
        assert_eq!(
            from_slice(b"\x92\xC3\x93\x01\x02\x03"),
            Ok((
                Test {
                    status: true,
                    point: [1, 2, 3]
                },
                6
            ))
        );
    }

    #[test]
    fn test_de_struct_with_tuple_field() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Test {
            status: bool,
            point: (u32, u32, u32),
        }

        assert_eq!(
            from_slice(b"\x82\xA6status\xC3\xA5point\x93\x01\x02\x03"),
            Ok((
                Test {
                    status: true,
                    point: (1, 2, 3)
                },
                19
            ))
        );
        assert_eq!(
            from_slice(b"\x82\x00\xC3\x01\x93\x01\x02\x03"),
            Ok((
                Test {
                    status: true,
                    point: (1, 2, 3)
                },
                8
            ))
        );
        assert_eq!(
            from_slice(b"\x92\xC3\x93\x01\x02\x03"),
            Ok((
                Test {
                    status: true,
                    point: (1, 2, 3)
                },
                6
            ))
        );
    }

    #[test]
    fn test_de_streaming() {
        let test = Test {
            compact: true,
            schema: 0,
            unit: Unit
        };
        let input = b"\xC0\xC2\x00\xA3ABC\xC4\x04_xyz\x83\xA7compact\xC3\xA6schema\x00\xA4unit\xC0\x93\x01\x02\x03\xC0";
        let (res, input) = from_slice_split_tail::<()>(input).unwrap();
        assert_eq!(res, ());
        let (res, input) = from_slice_split_tail::<bool>(input).unwrap();
        assert_eq!(res, false);
        let (res, input) = from_slice_split_tail::<i8>(input).unwrap();
        assert_eq!(res, 0);
        let (res, input) = from_slice_split_tail::<&str>(input).unwrap();
        assert_eq!(res, "ABC");
        let (res, input) = from_slice_split_tail::<&[u8]>(input).unwrap();
        assert_eq!(res, b"_xyz");
        let (res, input) = from_slice_split_tail::<Test>(input).unwrap();
        assert_eq!(res, test);
        let (res, input) = from_slice_split_tail::<[u32;3]>(input).unwrap();
        assert_eq!(res, [1,2,3]);
        let (res, input) = from_slice_split_tail::<Option<()>>(input).unwrap();
        assert_eq!(res, None);
        assert_eq!(input, b"");
        // error
        assert_eq!(from_slice_split_tail::<()>(input), Err(Error::UnexpectedEof));
    }

    #[test]
    fn test_de_ignoring_extra_fields() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Temperature {
            temp: u32,
        }
        let input = &[
            0x8F,
            0xA4,b't',b'e',b'm',b'p', 20,
            0xA1,b'n', 0xC0,
            0xA1,b't', 0xC2,
            0xA1,b'f', 0xC3,
            0xA4,b'f',b'i',b'x',b'+', 0x7F,
            0xA4,b'f',b'i',b'x',b'-', -32i8 as _,
            0xA2,b'u',b'8',      0xCC,0xFF,
            0xA3,b'u',b'1',b'6', 0xCD,0xFF,0xFF,
            0xA3,b'u',b'3',b'2', 0xCE,0xFF,0xFF,0xFF,0xFF,
            0xA3,b'u',b'6',b'4', 0xCF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,
            0xA2,b'i',b'8',      0xD0,0xFF,
            0xA3,b'i',b'1',b'6', 0xD1,0xFF,0xFF,
            0xA3,b'i',b'3',b'2', 0xD2,0xFF,0xFF,0xFF,0xFF,
            0xA3,b'i',b'6',b'4', 0xD3,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,
            0xA3,b's',b't',b'r', 0xBF, b'J',b'a',b'c',b'k',b'd',b'a',b'w',b's',
                                       b'l',b'o',b'v',b'e',
                                       b'm',b'y',
                                       b'b',b'i',b'g',
                                       b's',b'p',b'h',b'i',b'n',b'x',
                                       b'o',b'f',
                                       b'q',b'u',b'a',b'r',b't',b'z'
        ];
        assert_eq!(
            from_slice(input),
            Ok((Temperature { temp: 20 }, input.len()))
        );
        let input = &[
            0x8F,
            0xA1,b'n', 0xC0,
            0xA1,b't', 0xC2,
            0xA1,b'f', 0xC3,
            0xA4,b'f',b'i',b'x',b'+', 0x7F,
            0xA4,b'f',b'i',b'x',b'-', -32i8 as _,
            0xA2,b'u',b'8',      0xCC,0xFF,
            0xA3,b'u',b'1',b'6', 0xCD,0xFF,0xFF,
            0xA3,b'u',b'3',b'2', 0xCE,0xFF,0xFF,0xFF,0xFF,
            0xA3,b'u',b'6',b'4', 0xCF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,
            0xA2,b'i',b'8',      0xD0,0xFF,
            0xA3,b'i',b'1',b'6', 0xD1,0xFF,0xFF,
            0xA3,b'i',b'3',b'2', 0xD2,0xFF,0xFF,0xFF,0xFF,
            0xA3,b'i',b'6',b'4', 0xD3,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,
            0xA3,b's',b't',b'r', 0xBF, b'J',b'a',b'c',b'k',b'd',b'a',b'w',b's',
                                       b'l',b'o',b'v',b'e',
                                       b'm',b'y',
                                       b'b',b'i',b'g',
                                       b's',b'p',b'h',b'i',b'n',b'x',
                                       b'o',b'f',
                                       b'q',b'u',b'a',b'r',b't',b'z',
            0xA4,b't',b'e',b'm',b'p', 20
        ];
        assert_eq!(
            from_slice(input),
            Ok((Temperature { temp: 20 }, input.len()))
        );
        let input = &[
            0x89,
            0xA4,b't',b'e',b'm',b'p', 0xCC, 220,
            0xA3,b'f',b'3',b'2', 0xCA,0x00,0x00,0x00,0x00,
            0xA3,b'f',b'6',b'4', 0xCB,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
            0xA2,b's',b'8', 0xD9,0x01,b'-',
            0xA3,b's',b'1',b'6', 0xDA,0x00,0x01,b'-',
            0xA3,b's',b'3',b'2', 0xDB,0x00,0x00,0x00,0x01,b'-',
            0xA2,b'b',b'8', 0xC4,0x01,0x80,
            0xA3,b'b',b'1',b'6', 0xC5,0x00,0x01,0x80,
            0xA3,b'b',b'3',b'2', 0xC6,0x00,0x00,0x00,0x01,0x80,
        ];
        assert_eq!(
            from_slice(input),
            Ok((Temperature { temp: 220 }, input.len()))
        );
        let input = &[
            0x89,
            0xA1,b'a', 0x90,
            0xA2,b'a',b'1', 0x91,0x00,
            0xA2,b'a',b's', 0xDC,0x00,0x02, 0xA0, 0xA3,b'1',b'2',b'3',
            0xA2,b'a',b'l', 0xDD,0x00,0x00,0x00,0x02, 0xA0, 0xA3,b'1',b'2',b'3',
            0xA1,b'm', 0x80,
            0xA2,b'm',b'1', 0x81,0x00,0xA0,
            0xA2,b'm',b's', 0xDE,0x00,0x02, 0x00,0xA0, 0x01,0xA3,b'1',b'2',b'3',
            0xA2,b'm',b'l', 0xDF,0x00,0x00,0x00,0x02, 0xA1,b'x', 0x92,0xC2,0xC3,
                                                      0xA1,b'y', 0x91,0xC0,
            0xA4,b't',b'e',b'm',b'p', 0xCC, 220,
        ];
        assert_eq!(
            from_slice(input),
            Ok((Temperature { temp: 220 }, input.len()))
        );
        let input = &[
            0x8B,
            0xA3,b'f',b'3',b'2', 0xCA,0x00,0x00,0x00,0x00,
            0xA3,b'f',b'6',b'4', 0xCB,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
            0xA2,b'e',b'8', 0xC7,0x01,0x7F,b'.',
            0xA3,b'e',b'1',b'6', 0xC8,0x00,0x01,0x7F,b'.',
            0xA3,b'e',b'3',b'2', 0xC9,0x00,0x00,0x00,0x01,0x7F,b'.',
            0xA2,b'x',b'1', 0xD4,0x7F,b'.',
            0xA2,b'x',b'2', 0xD5,0x7F,b'.',b'.',
            0xA2,b'x',b'4', 0xD6,0x7F,b'.',b'.',b'.',b'.',
            0xA2,b'x',b'8', 0xD7,0x7F,b'.',b'.',b'.',b'.',b'.',b'.',b'.',b'.',
            0xA3,b'x',b'1',b'6', 0xD8,0x7F,b'.',b'.',b'.',b'.',b'.',b'.',b'.',b'.',
                                           b'.',b'.',b'.',b'.',b'.',b'.',b'.',b'.',
            0xA4,b't',b'e',b'm',b'p', 0xCD,2,8,
        ];
        assert_eq!(
            from_slice(input),
            Ok((Temperature { temp: 520 }, input.len()))
        );
        assert_eq!(
            from_slice::<Temperature>(&[
                0x82,
                0xA4,b't',b'e',b'm',b'p', 20,
                0xA1,b'_', 0xC1
            ]),
            Err(Error::ReservedCode)
        );
    }

    #[test]
    fn test_de_any() {
        #[derive(Debug, Deserialize, PartialEq)]
        #[serde(untagged)]
        enum Thing<'a> {
            Nope,
            Bool(bool),
            Str(&'a str),
            Bytes(&'a[u8]),
            Uint(u32),
            Int(i32),
            LongUint(u64),
            LongInt(i64),
            Float(f64),
            Array([&'a str;2]),
            Map{ a: u32, b: &'a str},
        }
        let input = b"\xC0";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Nope, input.len()))
        );
        let input = b"\xC2";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Bool(false), input.len()))
        );
        let input = b"\x00";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Uint(0), input.len()))
        );
        let input = b"\xFF";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Int(-1), input.len())));
        let input = b"\xA3foo";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Str("foo"), input.len())));
        let input = b"\xD9\x03foo";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Str("foo"), input.len())));
        let input = b"\xDA\x00\x03foo";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Str("foo"), input.len())));
        let input = b"\xDB\x00\x00\x00\x03foo";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Str("foo"), input.len())));
        let input = b"\xC4\x01\x80";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Bytes(b"\x80"), input.len())));
        let input = b"\xCC\x00";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Uint(0), input.len())));
        let input = b"\xCD\x00\x00";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Uint(0), input.len())));
        let input = b"\xCE\x00\x00\x00\x00";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Uint(0), input.len())));
        let input = b"\xCF\x00\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Uint(0), input.len())));
        let input = b"\xCF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF";
        assert_eq!(
            from_slice(input),
            Ok((Thing::LongUint(u64::MAX), input.len())));
        let input = b"\xD3\x00\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Uint(0), input.len())));
        let input = b"\xD0\xFF";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Int(-1), input.len())));
        let input = b"\xD1\xFF\xFF";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Int(-1), input.len())));
        let input = b"\xD2\xFF\xFF\xFF\xFF";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Int(-1), input.len())));
        let input = b"\xD3\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Int(-1), input.len())));
        let input = b"\xD3\x80\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(
            from_slice(input),
            Ok((Thing::LongInt(i64::MIN), input.len())));
        let input = b"\xCA\x00\x00\x00\x00";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Float(0.0), input.len())));
        let input = b"\xCB\x00\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Float(0.0), input.len())));
        let input = b"\xCB\x7F\xEF\xFF\xFF\xFF\xFF\xFF\xFF";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Float(f64::MAX), input.len())));
        let input = b"\x92\xA2xy\xA3abc";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Array(["xy","abc"]), input.len())));
        let input = b"\xDC\x00\x02\xA2xy\xA3abc";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Array(["xy","abc"]), input.len())));
        let input = b"\xDD\x00\x00\x00\x02\xA2xy\xA3abc";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Array(["xy","abc"]), input.len())));
        let input = b"\x82\xA1a\x7e\xA1b\xA3zyx";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Map{a:126,b:"zyx"}, input.len())));
        let input = b"\xDE\x00\x02\xA1a\x7e\xA1b\xA3zyx";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Map{a:126,b:"zyx"}, input.len())));
        let input = b"\xDF\x00\x00\x00\x02\xA1a\x7e\xA1b\xA3zyx";
        assert_eq!(
            from_slice(input),
            Ok((Thing::Map{a:126,b:"zyx"}, input.len())));
        // error
        assert_eq!(from_slice::<Thing>(b""), Err(Error::UnexpectedEof));
        assert_eq!(from_slice::<Thing>(b"\xC1"), Err(Error::ReservedCode));
        assert_eq!(from_slice::<Thing>(b"\xC7"), Err(Error::UnsupportedExt));
        assert_eq!(from_slice::<Thing>(b"\xC8"), Err(Error::UnsupportedExt));
        assert_eq!(from_slice::<Thing>(b"\xC9"), Err(Error::UnsupportedExt));
        assert_eq!(from_slice::<Thing>(b"\xD4"), Err(Error::UnsupportedExt));
        assert_eq!(from_slice::<Thing>(b"\xD5"), Err(Error::UnsupportedExt));
        assert_eq!(from_slice::<Thing>(b"\xD6"), Err(Error::UnsupportedExt));
        assert_eq!(from_slice::<Thing>(b"\xD7"), Err(Error::UnsupportedExt));
        assert_eq!(from_slice::<Thing>(b"\xD8"), Err(Error::UnsupportedExt));
    }

    #[test]
    fn test_de_ignore_err() {
        assert_eq!(Deserializer::from_slice(b"").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\x81").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\x81\xC0").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\x91").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xA1").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xC4").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xC4\x01").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xC5\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xC5\x00\x01").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xC6\x00\x00\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xC6\x00\x00\x00\x01").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xC7").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xC7\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xC7\x01\x7f").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xC8").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xC8\x00\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xC8\x00\x01\x7f").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xC9").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xC9\x00\x00\x00\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xC9\x00\x00\x00\x01\x7f").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xCA").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xCA\x00\x00\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xCB").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xCB\x00\x00\x00\x00\x00\x00\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xCC").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xCD\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xCE\x00\x00\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xCF\x00\x00\x00\x00\x00\x00\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD0").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD1\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD2\x00\x00\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD3\x00\x00\x00\x00\x00\x00\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD4").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD4\x7f").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD5").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD5\x7f").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD5\x7f\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD6").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD6\x7f").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD6\x7f\x00\x00\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD7").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD7\x7f").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD7\x7f\x00\x00\x00\x00\x00\x00\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD8").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD8\x7f").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD8\x7f\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD9").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xD9\x01").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDA\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDA\x00\x01").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDB\x00\x00\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDB\x00\x00\x00\x01").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDC").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDC\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDC\x00\x01").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDD").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDD\x00\x00\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDD\x00\x00\x00\x01").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDE").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDE\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDE\x00\x01").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDE\x00\x01\xC0").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDF").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDF\x00\x00\x00").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDF\x00\x00\x00\x01").eat_message(), Err(Error::UnexpectedEof));
        assert_eq!(Deserializer::from_slice(b"\xDF\x00\x00\x00\x01\xC0").eat_message(), Err(Error::UnexpectedEof));
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn test_de_error_string() {
        assert_eq!(&format!("{}", Error::UnexpectedEof), "Unexpected end of MessagePack input");
        assert_eq!(&format!("{}", Error::ReservedCode), "Reserved MessagePack code in input");
        assert_eq!(&format!("{}", Error::UnsupportedExt), "Unsupported MessagePack extension code in input");
        assert_eq!(&format!("{}", Error::InvalidInteger), "Could not coerce integer to a deserialized type");
        assert_eq!(&format!("{}", Error::InvalidType), "Invalid type");
        assert_eq!(&format!("{}", Error::InvalidUnicodeCodePoint), "Invalid unicode code point");
        assert_eq!(&format!("{}", Error::ExpectedInteger), "Expected MessagePack integer");
        assert_eq!(&format!("{}", Error::ExpectedNumber), "Expected MessagePack number");
        assert_eq!(&format!("{}", Error::ExpectedString), "Expected MessagePack string");
        assert_eq!(&format!("{}", Error::ExpectedBin), "Expected MessagePack bin");
        assert_eq!(&format!("{}", Error::ExpectedNil), "Expected MessagePack nil");
        assert_eq!(&format!("{}", Error::ExpectedArray), "Expected MessagePack array");
        assert_eq!(&format!("{}", Error::ExpectedMap), "Expected MessagePack map");
        assert_eq!(&format!("{}", Error::ExpectedStruct), "Expected MessagePack map or array");
        assert_eq!(&format!("{}", Error::ExpectedIdentifier), "Expected a struct field or enum variant identifier");
        assert_eq!(&format!("{}", Error::TrailingElements), "Too many elements for a deserialized type");
        assert_eq!(&format!("{}", Error::InvalidLength), "Invalid length");
        let custom: Error = serde::de::Error::custom("xxx");
        assert_eq!(format!("{}", custom), "xxx while deserializing MessagePack");
    }

    #[cfg(not(any(feature = "std", feature = "alloc")))]
    #[test]
    fn test_de_error_fmt() {
        use crate::ser_write::SliceWriter;
        use core::fmt::Write;
        let mut buf = [0u8;59];
        let mut writer = SliceWriter::new(&mut buf);
        let custom: Error = serde::de::Error::custom("xxx");
        write!(writer, "{}", custom).unwrap();
        assert_eq!(writer.as_ref(), "MessagePack does not match deserializer’s expected format".as_bytes());
    }
}
