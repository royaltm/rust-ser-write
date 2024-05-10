//! MessagePack serde serializer for `ser-write`
use core::fmt;

#[cfg(feature = "std")]
use std::{vec::Vec, string::ToString};

#[cfg(all(feature = "alloc",not(feature = "std")))]
use alloc::{vec::Vec, string::ToString};

use serde::{ser, Serialize};
use ser::Serializer as _;
// use ser_write::{SerResult as Result};
use super::magick::*;

use ser_write::{SerWrite, SerError};

/// MessagePack serializer serializing structs to arrays and enum variants as indexes
pub struct CompactSerializer<W> {
    output: W
}

/// MessagePack serializer serializing structs to maps with field names and enum variants as names
pub struct PortableSerializer<W> {
    output: W
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
/// Serialize `value` as a MessagePack message to a vector of bytes
///
/// Serialize data structures as arrays without field names and enum variants as indexes.
pub fn to_vec<T>(vec: &mut Vec<u8>, value: &T) -> Result<(), SerError>
    where T: Serialize
{
    to_writer(vec, value)
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
/// Serialize `value` as a MessagePack message to a vector of bytes
///
/// Serialize data structures as maps where resulting message will contain field and enum variant names.
pub fn to_vec_named<T>(vec: &mut Vec<u8>, value: &T) -> Result<(), SerError>
    where T: Serialize
{
    to_writer_named(vec, value)
}

/// Serialize `value` as a MessagePack message to a [`SerWrite`] implementation.
///
/// Serialize data structures as arrays without field names and enum variants as indexes.
pub fn to_writer<W, T>(writer: W, value: &T) -> Result<(), W::Error>
    where W: SerWrite,
          <W as SerWrite>::Error: fmt::Display+fmt::Debug,
          T: Serialize
{
    let mut serializer = CompactSerializer::new(writer);
    value.serialize(&mut serializer)
}

/// Serialize `value` as a MessagePack message to a [`SerWrite`] implementation.
///
/// Serialize data structures as maps where resulting message will contain field and enum variant names.
pub fn to_writer_named<W, T>(writer: W, value: &T) -> Result<(), W::Error>
    where W: SerWrite,
          <W as SerWrite>::Error: fmt::Display+fmt::Debug,
          T: Serialize
{
    let mut serializer = PortableSerializer::new(writer);
    value.serialize(&mut serializer)
}

/// Serializing error
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Error<E> {
    /// Writer error
    Writer(E),
    /// Undetermined map length or too many items
    MapLength,
    /// Undetermined sequence length or too many items
    SeqLength,
    /// Serializer could not determine string size
    StrLength,
    /// Serializer could not determine byte-array size
    DataLength,
    /// Error formatting a collected a string
    FormatError,
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
    /// An error passed down from a [`serde::ser::Serialize`] implementation
    SerializeError(std::string::String),
    #[cfg(not(any(feature = "std", feature = "alloc")))]
    SerializeError
}

/// Serialization result
pub type Result<T, E> = core::result::Result<T, Error<E>>;

impl<E: fmt::Display+fmt::Debug> serde::de::StdError for Error<E> {}

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Writer(err) => err.fmt(f),
            Error::MapLength => f.write_str("unknown or invalid map length"),
            Error::SeqLength => f.write_str("unknown or invalid sequence length"),
            Error::StrLength => f.write_str("invalid string length"),
            Error::DataLength => f.write_str("invalid byte array length"),
            Error::FormatError => f.write_str("error collecting a string"),
            #[cfg(any(feature = "std", feature = "alloc"))]
            Error::SerializeError(s) => write!(f, "{} while serializing JSON", s),
            #[cfg(not(any(feature = "std", feature = "alloc")))]
            Error::SerializeError => f.write_str("custom error while serializing JSON"),
        }
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<E: fmt::Display+fmt::Debug> serde::ser::Error for Error<E> {
    fn custom<T>(msg: T) -> Self
        where T: fmt::Display
    {
        Error::SerializeError(msg.to_string())
    }
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl<E: fmt::Display+fmt::Debug> serde::ser::Error for Error<E> {
    fn custom<T>(_msg: T) -> Self
        where T: fmt::Display
    {
        Error::SerializeError
    }
}

impl<E> From<E> for Error<E> {
    fn from(err: E) -> Self {
        Error::Writer(err)
    }
}

impl<W: SerWrite> CompactSerializer<W> {
    fn serialize_variant(&mut self, variant_index: u32, _variant_name: &'static str) -> Result<(), W::Error> {
        write_u32(&mut self.output, variant_index)
    }

    fn serialize_struct<'a>(&'a mut self, len: usize) -> Result<SerializeStructArray<'a, CompactSerializer<W>>, W::Error> {
        write_array_len(&mut self.output, len)?;
        Ok(SerializeStructArray { ser: self })
    }
}

impl<W: SerWrite> PortableSerializer<W> {
    fn serialize_variant(&mut self, _variant_index: u32, variant_name: &'static str) -> Result<(), W::Error> {
        write_str(&mut self.output, variant_name)
    }

    fn serialize_struct<'a>(&'a mut self, len: usize) -> Result<SerializeStructMap<'a, PortableSerializer<W>>, W::Error> {
        write_map_len(&mut self.output, len)?;
        Ok(SerializeStructMap { ser: self })
    }
}

macro_rules! implement_serializer {
    ($serializer:ident, $struct_serializer:ident) => {

impl<W> $serializer<W> {
    /// Create a new `Serializer` with the given `output` that should implement [`SerWrite`].
    #[inline(always)]
    pub fn new(output: W) -> Self {
        $serializer { output }
    }
    /// Destruct self returning the `output` object.
    #[inline(always)]
    pub fn into_inner(self) -> W {
        self.output
    }
    /// Provide access to the inner writer.
    #[inline(always)]
    pub fn writer(&mut self) -> &mut W {
        &mut self.output
    }
}

impl<'a, W: SerWrite> ser::Serializer for &'a mut $serializer<W>
    where <W as SerWrite>::Error: fmt::Display+fmt::Debug
{
    type Ok = ();
    type Error = Error<W::Error>;

    type SerializeSeq = SerializeSeqMap<'a, $serializer<W>>;
    type SerializeTuple = SerializeTuple<'a, $serializer<W>>;
    type SerializeTupleStruct = SerializeTuple<'a, $serializer<W>>;
    type SerializeTupleVariant = SerializeTuple<'a, $serializer<W>>;
    type SerializeMap = SerializeSeqMap<'a, $serializer<W>>;
    type SerializeStruct = $struct_serializer<'a, $serializer<W>>;
    type SerializeStructVariant = $struct_serializer<'a, $serializer<W>>;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn serialize_bool(self, v: bool) -> Result<(), W::Error> {
        Ok(self.output.write_byte(if v { TRUE } else { FALSE })?)
    }
    #[inline(always)]
    fn serialize_i8(self, v: i8) -> Result<(), W::Error> {
        if v >= MIN_NEGFIXINT {
            self.output.write_byte(v as u8)?;
        }
        else {
            self.output.write_byte(INT_8)?;
            self.output.write_byte(v as u8)?;
        }
        Ok(())
    }
    #[inline(always)]
    fn serialize_i16(self, v: i16) -> Result<(), W::Error> {
        if FIXINT_I16.contains(&v) {
            self.output.write_byte(v as u8)?;
        }
        else if let Ok(v) = i8::try_from(v) {
            self.output.write_byte(INT_8)?;
            self.output.write_byte(v as u8)?;
        }
        else if let Ok(v) = u8::try_from(v) {
            self.output.write_byte(UINT_8)?;
            self.output.write_byte(v)?;
        }
        else {
            self.output.write_byte(INT_16)?;
            self.output.write(&v.to_be_bytes())?;
        }
        Ok(())
    }
    #[inline]
    fn serialize_i32(self, v: i32) -> Result<(), W::Error> {
        if FIXINT_I32.contains(&v) {
            self.output.write_byte(v as u8)?;
        }
        else if let Ok(v) = i8::try_from(v) {
            self.output.write_byte(INT_8)?;
            self.output.write_byte(v as u8)?;
        }
        else if let Ok(v) = u8::try_from(v) {
            self.output.write_byte(UINT_8)?;
            self.output.write_byte(v)?;
        }
        else if let Ok(v) = i16::try_from(v) {
            self.output.write_byte(INT_16)?;
            self.output.write(&v.to_be_bytes())?;
        }
        else if let Ok(v) = u16::try_from(v) {
            self.output.write_byte(UINT_16)?;
            self.output.write(&v.to_be_bytes())?;
        }
        else {
            self.output.write_byte(INT_32)?;
            self.output.write(&v.to_be_bytes())?;
        }
        Ok(())
    }

    fn serialize_i64(self, v: i64) -> Result<(), W::Error> {
        if FIXINT_I64.contains(&v) {
            self.output.write_byte(v as u8)?;
        }
        else if let Ok(v) = i8::try_from(v) {
            self.output.write_byte(INT_8)?;
            self.output.write_byte(v as u8)?;
        }
        else if let Ok(v) = u8::try_from(v) {
            self.output.write_byte(UINT_8)?;
            self.output.write_byte(v)?;
        }
        else if let Ok(v) = i16::try_from(v) {
            self.output.write_byte(INT_16)?;
            self.output.write(&v.to_be_bytes())?;
        }
        else if let Ok(v) = u16::try_from(v) {
            self.output.write_byte(UINT_16)?;
            self.output.write(&v.to_be_bytes())?;
        }
        else if let Ok(v) = i32::try_from(v) {
            self.output.write_byte(INT_32)?;
            self.output.write(&v.to_be_bytes())?;
        }
        else if let Ok(v) = u32::try_from(v) {
            self.output.write_byte(UINT_32)?;
            self.output.write(&v.to_be_bytes())?;
        }
        else {
            self.output.write_byte(INT_64)?;
            self.output.write(&v.to_be_bytes())?;
        }
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<(), W::Error> {
        if v <= MAX_POSFIXINT {
            self.output.write_byte(v)?;
        }
        else {
            self.output.write_byte(UINT_8)?;
            self.output.write_byte(v)?;
        }
        Ok(())
    }

    fn serialize_u16(self, v: u16) -> Result<(), W::Error> {
        if v <= MAX_POSFIXINT as u16 {
            self.output.write_byte(v as u8)?;
        }
        else if let Ok(v) = u8::try_from(v) {
            self.output.write_byte(UINT_8)?;
            self.output.write_byte(v)?;
        }
        else {
            self.output.write_byte(UINT_16)?;
            self.output.write(&v.to_be_bytes())?;
        }
        Ok(())
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, W::Error> {
        write_u32(&mut self.output, v)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, W::Error> {
        if v <= MAX_POSFIXINT as u64 {
            self.output.write_byte(v as u8)?;
        }
        else if let Ok(v) = u8::try_from(v) {
            self.output.write_byte(UINT_8)?;
            self.output.write_byte(v)?;
        }
        else if let Ok(v) = u16::try_from(v) {
            self.output.write_byte(UINT_16)?;
            self.output.write(&v.to_be_bytes())?;
        }
        else if let Ok(v) = u32::try_from(v) {
            self.output.write_byte(UINT_32)?;
            self.output.write(&v.to_be_bytes())?;
        }
        else {
            self.output.write_byte(UINT_64)?;
            self.output.write(&v.to_be_bytes())?;
        }
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<(), W::Error> {
        self.output.write_byte(FLOAT_32)?;
        Ok(self.output.write(&v.to_be_bytes())?)
    }

    fn serialize_f64(self, v: f64) -> Result<(), W::Error> {
        self.output.write_byte(FLOAT_64)?;
        Ok(self.output.write(&v.to_be_bytes())?)
    }

    fn serialize_char(self, v: char) -> Result<(), W::Error> {
        let mut encoding_tmp = [0u8; 4];
        let encoded = v.encode_utf8(&mut encoding_tmp);
        self.serialize_str(encoded)
    }

    fn serialize_str(self, v: &str) -> Result<(), W::Error> {
        Ok(write_str(&mut self.output, v)?)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<(), W::Error> {
        let size = v.len();
        if let Ok(size) = u8::try_from(size) {
            self.output.write_byte(BIN_8)?;
            self.output.write_byte(size)?;
        }
        else if let Ok(size) = u16::try_from(size) {
            self.output.write_byte(BIN_16)?;
            self.output.write(&size.to_be_bytes())?;
        }
        else if let Ok(size) = u32::try_from(size) {
            self.output.write_byte(BIN_32)?;
            self.output.write(&size.to_be_bytes())?;
        }
        else {
            return Err(Error::DataLength)
        }
        Ok(self.output.write(v)?)
    }

    fn serialize_none(self) -> Result<(), W::Error> {
        Ok(self.output.write_byte(NIL)?)
    }

    fn serialize_some<T>(self, value: &T) -> Result<(), W::Error>
        where T: ?Sized + Serialize
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), W::Error> {
        self.serialize_none()
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), W::Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<(), W::Error> {
        self.serialize_variant(variant_index, variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), W::Error>
        where T: ?Sized + Serialize
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), W::Error>
    where
        T: ?Sized + Serialize,
    {
        self.output.write_byte(FIXMAP|1)?;
        self.serialize_variant(variant_index, variant)?;
        value.serialize(&mut *self)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, W::Error> {
        let len = len.ok_or(Error::SeqLength)?;
        write_array_len(&mut self.output, len)?;
        Ok(SerializeSeqMap { len, ser: self })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, W::Error> {
        write_array_len(&mut self.output, len)?;
        Ok(SerializeTuple { ser: self })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, W::Error> {
        self.serialize_tuple(len)
    }

    // Tuple variants are represented in JSON as `{ NAME: [ ... ] }`.
    // This is the externally tagged representation.
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, W::Error> {
        self.output.write_byte(FIXMAP|1)?;
        self.serialize_variant(variant_index, variant)?;
        self.serialize_tuple(len)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, W::Error> {
        let len = len.ok_or(Error::MapLength)?;
        write_map_len(&mut self.output, len)?;
        Ok(SerializeSeqMap { len, ser: self })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, W::Error> {
        self.serialize_struct(len)
    }

    // Struct variants are represented in JSON as `{ NAME: { K: V, ... } }`.
    // This is the externally tagged representation.
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, W::Error> {
        self.output.write_byte(FIXMAP|1)?;
        self.serialize_variant(variant_index, variant)?;
        self.serialize_struct(len)
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    fn collect_str<T: ?Sized>(self, value: &T) -> Result<Self::Ok, W::Error>
        where T: fmt::Display
    {
        self.serialize_str(&value.to_string())
    }

    #[cfg(not(any(feature = "std", feature = "alloc")))]
    /// This implementation will format the value string twice, once to establish its size and later to actually
    /// write the string.
    fn collect_str<T: ?Sized>(self, value: &T) -> Result<Self::Ok, W::Error>
        where T: fmt::Display
    {
        if let Some(s) = format_args!("{}", value).as_str() {
            return self.serialize_str(s)
        }
        let mut col = StringLenCounter(0);
        fmt::write(&mut col, format_args!("{}", value)).map_err(|_| Error::FormatError)?;
        let StringLenCounter(len) = col;
        write_str_len(&mut self.output, len)?;
        let mut col = StringCollector::new(len, &mut self.output);
        fmt::write(&mut col, format_args!("{}", value)).map_err(|_| Error::FormatError)
    }
}

};
} /* implement_serializer */

implement_serializer!(CompactSerializer, SerializeStructArray);
implement_serializer!(PortableSerializer, SerializeStructMap);

#[inline]
fn write_u32<W: SerWrite>(output: &mut W, v: u32) -> Result<(), W::Error> {
    if v <= MAX_POSFIXINT as u32 {
        output.write_byte(v as u8)?;
    }
    else if let Ok(v) = u8::try_from(v) {
        output.write_byte(UINT_8)?;
        output.write_byte(v)?;
    }
    else if let Ok(v) = u16::try_from(v) {
        output.write_byte(UINT_16)?;
        output.write(&v.to_be_bytes())?;
    }
    else {
        output.write_byte(UINT_32)?;
        output.write(&v.to_be_bytes())?;
    }
    Ok(())
}

#[inline]
fn write_str<W: SerWrite>(output: &mut W, v: &str) -> Result<(), W::Error> {
    let size = v.len();
    write_str_len(output, size)?;
    Ok(output.write_str(v)?)
}

#[inline]
fn write_str_len<W: SerWrite>(output: &mut W, len: usize) -> Result<(), W::Error> {
    if len <= MAX_FIXSTR_SIZE {
        output.write_byte(FIXSTR | (len as u8))?;
    }
    else if let Ok(len) = u8::try_from(len) {
        output.write_byte(STR_8)?;
        output.write_byte(len)?;
    }
    else if let Ok(len) = u16::try_from(len) {
        output.write_byte(STR_16)?;
        output.write(&len.to_be_bytes())?;
    }
    else if let Ok(len) = u32::try_from(len) {
        output.write_byte(STR_32)?;
        output.write(&len.to_be_bytes())?;
    }
    else {
        return Err(Error::StrLength)
    }
    Ok(())
}

#[inline]
fn write_array_len<W: SerWrite>(output: &mut W, len: usize) -> Result<(), W::Error> {
    if len <= MAX_FIXARRAY_SIZE {
        output.write_byte(FIXARRAY | (len as u8))?;
    }
    else if let Ok(len) = u16::try_from(len) {
        output.write_byte(ARRAY_16)?;
        output.write(&len.to_be_bytes())?;
    }
    else if let Ok(len) = u32::try_from(len) {
        output.write_byte(ARRAY_32)?;
        output.write(&len.to_be_bytes())?;
    }
    else {
        return Err(Error::SeqLength)
    }
    Ok(())
}

#[inline]
fn write_map_len<W: SerWrite>(output: &mut W, len: usize) -> Result<(), W::Error> {
    if len <= MAX_FIXMAP_SIZE {
        output.write_byte(FIXMAP | (len as u8))?;
    }
    else if let Ok(len) = u16::try_from(len) {
        output.write_byte(MAP_16)?;
        output.write(&len.to_be_bytes())?;
    }
    else if let Ok(len) = u32::try_from(len) {
        output.write_byte(MAP_32)?;
        output.write(&len.to_be_bytes())?;
    }
    else {
        return Err(Error::MapLength)
    }
    Ok(())
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
struct StringLenCounter(usize);

#[cfg(not(any(feature = "std", feature = "alloc")))]
struct StringCollector<'a, W> {
    len: usize,
    output: &'a mut W,
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl<'a, W> StringCollector<'a, W> {
    #[inline(always)]
    fn new(len: usize, output: &'a mut W) -> Self {
        Self { len, output }
    }
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl fmt::Write for StringLenCounter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0 = self.0.checked_add(s.len()).ok_or_else(|| fmt::Error)?;
        Ok(())
    }
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl<'a, W: SerWrite> fmt::Write for StringCollector<'a, W> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.len = self.len.checked_sub(s.len()).ok_or_else(|| fmt::Error)?;
        self.output.write_str(s).map_err(|_| fmt::Error)
    }
}

pub struct SerializeSeqMap<'a, S> {
    len: usize,
    ser: &'a mut S
}

pub struct SerializeTuple<'a, S> {
    ser: &'a mut S
}

pub struct SerializeStructMap<'a, S> {
    ser: &'a mut S
}

pub struct SerializeStructArray<'a, S> {
    ser: &'a mut S
}

// This impl is SerializeSeq so these methods are called after `serialize_seq`
// is called on the Serializer.
impl<'a, S, E> ser::SerializeSeq for SerializeSeqMap<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = Error<E>>,
          E: fmt::Display + fmt::Debug
{
    type Ok = ();
    type Error = Error<E>;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), E>
        where T: ?Sized + Serialize
    {
        self.len = self.len.checked_sub(1).ok_or(Error::SeqLength)?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), E> {
        (self.len == 0).then_some(()).ok_or(Error::SeqLength)
    }
}

impl<'a, S, E> ser::SerializeTuple for SerializeTuple<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = Error<E>>,
          E: fmt::Display + fmt::Debug
{
    type Ok = ();
    type Error = Error<E>;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), E>
    where T: ?Sized + Serialize
    {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), E> {
        Ok(())
    }
}

impl<'a, S, E> ser::SerializeTupleStruct for SerializeTuple<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = Error<E>>,
          E: fmt::Display + fmt::Debug
{
    type Ok = ();
    type Error = Error<E>;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), E>
        where T: ?Sized + Serialize
    {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), E> {
        Ok(())
    }
}

// Tuple variants are a little different. { NAME: [ ... ]}
impl<'a, S, E> ser::SerializeTupleVariant for SerializeTuple<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = Error<E>>,
          E: fmt::Display + fmt::Debug
{
    type Ok = ();
    type Error = Error<E>;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), E>
    where T: ?Sized + Serialize
    {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), E> {
        Ok(())
    }
}

impl<'a, S, E> ser::SerializeMap for SerializeSeqMap<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = Error<E>>,
          E: fmt::Display + fmt::Debug
{
    type Ok = ();
    type Error = Error<E>;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), E>
        where T: ?Sized + Serialize
    {
        self.len = self.len.checked_sub(1).ok_or(Error::MapLength)?;
        key.serialize(&mut *self.ser)
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), E>
    where T: ?Sized + Serialize
    {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), E> {
        (self.len == 0).then_some(()).ok_or(Error::MapLength)
    }
}

impl<'a, S, E> ser::SerializeStruct for SerializeStructArray<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = Error<E>>,
          E: fmt::Display + fmt::Debug
{
    type Ok = ();
    type Error = Error<E>;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), E>
        where T: ?Sized + Serialize
    {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), E> {
        Ok(())
    }
}

impl<'a, S, E> ser::SerializeStructVariant for SerializeStructArray<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = Error<E>>,
          E: fmt::Display + fmt::Debug
{
    type Ok = ();
    type Error = Error<E>;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), E>
        where T: ?Sized + Serialize
    {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), E> {
        Ok(())
    }
}

impl<'a, S, E> ser::SerializeStruct for SerializeStructMap<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = Error<E>>,
          E: fmt::Display + fmt::Debug
{
    type Ok = ();
    type Error = Error<E>;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), E>
        where T: ?Sized + Serialize
    {
        self.ser.serialize_str(key)?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), E> {
        Ok(())
    }
}

impl<'a, S, E> ser::SerializeStructVariant for SerializeStructMap<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = Error<E>>,
          E: fmt::Display + fmt::Debug
{
    type Ok = ();
    type Error = Error<E>;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), E>
        where T: ?Sized + Serialize
    {
        self.ser.serialize_str(key)?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), E> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ser_msgpack() {
        #[derive(Serialize)]
        enum Flavors {
            Vanilla,
            Chocolate,
            Strawberry
        }
        #[derive(Serialize)]
        enum Prices<'a> {
            Vanilla(f32),
            Chocolate(&'a str),
            Strawberry { gold: u8, silver: u16 }
        }
        #[derive(Serialize)]
        struct Unit;
        #[derive(Serialize)]
        struct Test {
            compact: bool,
            schema: u32,
            unit: Unit
        }

        let test = Test {
            compact: true,
            schema: 0,
            unit: Unit
        };
        let expected = b"\x83\xA7compact\xC3\xA6schema\x00\xA4unit\xC0";
        let mut vec = Vec::new();
        to_vec_named(&mut vec, &test).unwrap();
        assert_eq!(&vec, expected);
        let expected = b"\x93\xC3\x00\xC0";
        vec.clear();
        to_vec(&mut vec, &test).unwrap();
        assert_eq!(&vec, expected);

        let test = [Flavors::Strawberry, Flavors::Vanilla, Flavors::Chocolate];
        let expected = b"\x93\xAAStrawberry\xA7Vanilla\xA9Chocolate";
        vec.clear();
        to_vec_named(&mut vec, &test).unwrap();
        assert_eq!(&vec, expected);
        let expected = b"\x93\x02\x00\x01";
        vec.clear();
        to_vec(&mut vec, &test).unwrap();
        assert_eq!(&vec, expected);

        let test = (Prices::Strawberry { gold: 7, silver: 1000 },
                    Prices::Vanilla(12.5),
                    Prices::Chocolate("free"));
        let expected = b"\x93\x81\xAAStrawberry\x82\xA4gold\x07\xA6silver\xCD\x03\xE8\x81\xA7Vanilla\xCA\x41\x48\x00\x00\x81\xA9Chocolate\xA4free";
        vec.clear();
        to_vec_named(&mut vec, &test).unwrap();
        assert_eq!(&vec, expected);
        let expected = b"\x93\x81\x02\x92\x07\xCD\x03\xE8\x81\x00\xCA\x41\x48\x00\x00\x81\x01\xA4free";
        vec.clear();
        to_vec(&mut vec, &test).unwrap();
        assert_eq!(&vec, expected);
    }
}
