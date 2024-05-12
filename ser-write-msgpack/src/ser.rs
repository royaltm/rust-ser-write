//! MessagePack serde serializer for `ser-write`
use core::fmt;

#[cfg(feature = "std")]
use std::{vec::Vec, string::{String, ToString}};

#[cfg(all(feature = "alloc",not(feature = "std")))]
use alloc::{vec::Vec, string::{String, ToString}};

use serde::{ser, Serialize};
use ser::Serializer as _;

use super::magick::*;

use ser_write::SerWrite;

/// MessagePack serializer serializing structs to arrays and enum variants as indexes.
///
/// **Warning**: with this serializer only last fields can be skipped from a data structure.
pub struct CompactSerializer<W> {
    output: W
}

/// MessagePack serializer serializing structs to maps with fields and enum variants as indexes
pub struct StructMapIdxSerializer<W> {
    output: W
}

/// MessagePack serializer serializing structs to maps with field names and enum variants as names
pub struct StructMapStrSerializer<W> {
    output: W
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub fn to_vec<T>(value: &T) -> Result<Vec<u8>, ser_write::SerError>
    where T: Serialize + ?Sized
{
    let mut vec = Vec::new();
    to_writer(&mut vec, value)?;
    Ok(vec)
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub fn to_vec_compact<T>(value: &T) -> Result<Vec<u8>, ser_write::SerError>
    where T: Serialize + ?Sized
{
    let mut vec = Vec::new();
    to_writer_compact(&mut vec, value)?;
    Ok(vec)
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub fn to_vec_named<T>(value: &T) -> Result<Vec<u8>, ser_write::SerError>
    where T: Serialize + ?Sized
{
    let mut vec = Vec::new();
    to_writer_named(&mut vec, value)?;
    Ok(vec)
}

/// Serialize `value` as a MessagePack message to a [`SerWrite`] implementation.
///
/// Serialize data structures as arrays without field names and enum variants as indexes.
///
/// **Warning**: with this function only last fields can be skipped from a data structure.
pub fn to_writer_compact<W, T>(writer: W, value: &T) -> Result<(), W::Error>
    where W: SerWrite,
          <W as SerWrite>::Error: fmt::Display+fmt::Debug,
          T: Serialize + ?Sized
{
    let mut serializer = CompactSerializer::new(writer);
    value.serialize(&mut serializer)
}

/// Serialize `value` as a MessagePack message to a [`SerWrite`] implementation.
///
/// Serialize data structures as maps with field and enum variants as indexes.
pub fn to_writer<W, T>(writer: W, value: &T) -> Result<(), W::Error>
    where W: SerWrite,
          <W as SerWrite>::Error: fmt::Display+fmt::Debug,
          T: Serialize + ?Sized
{
    let mut serializer = StructMapIdxSerializer::new(writer);
    value.serialize(&mut serializer)
}

/// Serialize `value` as a MessagePack message to a [`SerWrite`] implementation.
///
/// Serialize data structures as maps where resulting message will contain field and enum variant names.
pub fn to_writer_named<W, T>(writer: W, value: &T) -> Result<(), W::Error>
    where W: SerWrite,
          <W as SerWrite>::Error: fmt::Display+fmt::Debug,
          T: Serialize + ?Sized
{
    let mut serializer = StructMapStrSerializer::new(writer);
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
    /// String size too large
    StrLength,
    /// Byte-array size too large
    DataLength,
    /// Skipped a field in a struct using compact serializer
    FieldSkipped,
    /// Error formatting a collected string
    FormatError,
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
    /// An error passed down from a [`serde::ser::Serialize`] implementation
    SerializeError(String),
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
            Error::FieldSkipped => f.write_str("skipped a field in a middle of struct"),
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

impl<W: SerWrite> StructMapIdxSerializer<W> {
    fn serialize_variant(&mut self, variant_index: u32, _variant_name: &'static str) -> Result<(), W::Error> {
        write_u32(&mut self.output, variant_index)
    }

    fn serialize_struct(&mut self, len: usize) -> Result<SerializeStructIntMap<'_, StructMapIdxSerializer<W>>, W::Error> {
        write_map_len(&mut self.output, len)?;
        Ok(SerializeStructIntMap { ser: self, len, idx: 0 })
    }
}

impl<W: SerWrite> CompactSerializer<W> {
    fn serialize_variant(&mut self, variant_index: u32, _variant_name: &'static str) -> Result<(), W::Error> {
        write_u32(&mut self.output, variant_index)
    }

    fn serialize_struct(&mut self, len: usize) -> Result<SerializeStructArray<'_, CompactSerializer<W>>, W::Error> {
        write_array_len(&mut self.output, len)?;
        Ok(SerializeStructArray { ser: self, len })
    }
}

impl<W: SerWrite> StructMapStrSerializer<W> {
    fn serialize_variant(&mut self, _variant_index: u32, variant_name: &'static str) -> Result<(), W::Error> {
        write_str(&mut self.output, variant_name)
    }

    fn serialize_struct(&mut self, len: usize) -> Result<SerializeStructStrMap<'_, StructMapStrSerializer<W>>, W::Error> {
        write_map_len(&mut self.output, len)?;
        Ok(SerializeStructStrMap { ser: self, len })
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
    type SerializeTuple = SerializeSeqMap<'a, $serializer<W>>;
    type SerializeTupleStruct = SerializeSeqMap<'a, $serializer<W>>;
    type SerializeTupleVariant = SerializeSeqMap<'a, $serializer<W>>;
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
        write_str(&mut self.output, v)
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
        Ok(SerializeSeqMap { len, ser: self })
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
    fn collect_str<T>(self, value: &T) -> Result<Self::Ok, W::Error>
        where T: fmt::Display + ?Sized
    {
        self.serialize_str(&value.to_string())
    }

    #[cfg(not(any(feature = "std", feature = "alloc")))]
    /// This implementation will format the value string twice, once to establish its size and later to actually
    /// write the string.
    fn collect_str<T>(self, value: &T) -> Result<Self::Ok, W::Error>
        where T: fmt::Display + ?Sized
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
implement_serializer!(StructMapIdxSerializer, SerializeStructIntMap);
implement_serializer!(StructMapStrSerializer, SerializeStructStrMap);

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
        self.0 = self.0.checked_add(s.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl<'a, W: SerWrite> fmt::Write for StringCollector<'a, W> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.len = self.len.checked_sub(s.len()).ok_or(fmt::Error)?;
        self.output.write_str(s).map_err(|_| fmt::Error)
    }
}

pub struct SerializeSeqMap<'a, S> {
    ser: &'a mut S,
    len: usize
}

pub struct SerializeStructArray<'a, S> {
    ser: &'a mut S,
    len: usize
}

pub struct SerializeStructIntMap<'a, S> {
    ser: &'a mut S,
    len: usize,
    idx: u32,
}

pub struct SerializeStructStrMap<'a, S> {
    ser: &'a mut S,
    len: usize
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

impl<'a, S, E> ser::SerializeTuple for SerializeSeqMap<'a, S>
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

impl<'a, S, E> ser::SerializeTupleStruct for SerializeSeqMap<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = Error<E>>,
          E: fmt::Display + fmt::Debug
{
    type Ok = ();
    type Error = Error<E>;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), E>
        where T: ?Sized + Serialize
    {
        self.len = self.len.checked_sub(1).ok_or(Error::SeqLength)?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), E> {
        (self.len == 0).then_some(()).ok_or(Error::SeqLength)
    }
}

// Tuple variants are a little different. { NAME: [ ... ]}
impl<'a, S, E> ser::SerializeTupleVariant for SerializeSeqMap<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = Error<E>>,
          E: fmt::Display + fmt::Debug
{
    type Ok = ();
    type Error = Error<E>;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), E>
    where T: ?Sized + Serialize
    {
        self.len = self.len.checked_sub(1).ok_or(Error::SeqLength)?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), E> {
        (self.len == 0).then_some(()).ok_or(Error::SeqLength)
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
        self.len = self.len.checked_sub(1).ok_or(Error::SeqLength)?;
        value.serialize(&mut *self.ser)
    }

    /// Allow skipping only last fields
    fn skip_field(&mut self, _key: &'static str) -> Result<(), E> {
        (self.len == 0).then_some(()).ok_or(Error::FieldSkipped)
    }

    fn end(self) -> Result<(), E> {
        (self.len == 0).then_some(()).ok_or(Error::SeqLength)
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
        self.len = self.len.checked_sub(1).ok_or(Error::SeqLength)?;
        value.serialize(&mut *self.ser)
    }

    /// Allow skipping only last fields
    fn skip_field(&mut self, _key: &'static str) -> Result<(), E> {
        (self.len == 0).then_some(()).ok_or(Error::FieldSkipped)
    }

    fn end(self) -> Result<(), E> {
        (self.len == 0).then_some(()).ok_or(Error::SeqLength)
    }
}

impl<'a, S, E> ser::SerializeStruct for SerializeStructIntMap<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = Error<E>>,
          E: fmt::Display + fmt::Debug
{
    type Ok = ();
    type Error = Error<E>;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), E>
        where T: ?Sized + Serialize
    {
        self.len = self.len.checked_sub(1).ok_or(Error::MapLength)?;
        let idx = self.idx;
        self.idx = idx.wrapping_add(1);
        self.ser.serialize_u32(idx)?;
        value.serialize(&mut *self.ser)
    }

    fn skip_field(&mut self, _key: &'static str) -> Result<(), E> {
        self.idx = self.idx.wrapping_add(1);
        Ok(())
    }

    fn end(self) -> Result<(), E> {
        (self.len == 0).then_some(()).ok_or(Error::MapLength)
    }
}

impl<'a, S, E> ser::SerializeStructVariant for SerializeStructIntMap<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = Error<E>>,
          E: fmt::Display + fmt::Debug
{
    type Ok = ();
    type Error = Error<E>;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), E>
        where T: ?Sized + Serialize
    {
        self.len = self.len.checked_sub(1).ok_or(Error::MapLength)?;
        let idx = self.idx;
        self.idx = idx.wrapping_add(1);
        self.ser.serialize_u32(idx)?;
        value.serialize(&mut *self.ser)
    }

    fn skip_field(&mut self, _key: &'static str) -> Result<(), E> {
        self.idx = self.idx.wrapping_add(1);
        Ok(())
    }

    fn end(self) -> Result<(), E> {
        (self.len == 0).then_some(()).ok_or(Error::MapLength)
    }
}

impl<'a, S, E> ser::SerializeStruct for SerializeStructStrMap<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = Error<E>>,
          E: fmt::Display + fmt::Debug
{
    type Ok = ();
    type Error = Error<E>;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), E>
        where T: ?Sized + Serialize
    {
        self.len = self.len.checked_sub(1).ok_or(Error::MapLength)?;
        self.ser.serialize_str(key)?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), E> {
        (self.len == 0).then_some(()).ok_or(Error::MapLength)
    }
}

impl<'a, S, E> ser::SerializeStructVariant for SerializeStructStrMap<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = Error<E>>,
          E: fmt::Display + fmt::Debug
{
    type Ok = ();
    type Error = Error<E>;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), E>
        where T: ?Sized + Serialize
    {
        self.len = self.len.checked_sub(1).ok_or(Error::MapLength)?;
        self.ser.serialize_str(key)?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), E> {
        (self.len == 0).then_some(()).ok_or(Error::MapLength)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "std")]
    use std::{vec, vec::Vec, collections::BTreeMap};
    #[cfg(all(feature = "alloc",not(feature = "std")))]
    use alloc::{vec, vec::Vec, collections::BTreeMap};
    use super::*;
    use ser_write::{SliceWriter, SerError};

    fn to_slice_compact<'a, T>(buf: &'a mut[u8], value: &T) -> Result<&'a[u8], SerError>
        where T: Serialize + ?Sized
    {
        let mut writer = SliceWriter::new(buf);
        to_writer_compact(&mut writer, value)?;
        Ok(writer.split().0)
    }

    fn to_slice<'a, T>(buf: &'a mut[u8], value: &T) -> Result<&'a[u8], SerError>
        where T: Serialize + ?Sized
    {
        let mut writer = SliceWriter::new(buf);
        to_writer(&mut writer, value)?;
        Ok(writer.split().0)
    }

    fn to_slice_named<'a, T>(buf: &'a mut[u8], value: &T) -> Result<&'a[u8], SerError>
        where T: Serialize + ?Sized
    {
        let mut writer = SliceWriter::new(buf);
        to_writer_named(&mut writer, value)?;
        Ok(writer.split().0)
    }

    #[test]
    fn test_msgpack() {
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
        let mut buf = [0u8;80];

        let expected = b"\x83\xA7compact\xC3\xA6schema\x00\xA4unit\xC0";
        assert_eq!(to_slice_named(&mut buf, &test).unwrap(), expected);
        let expected = b"\x83\x00\xC3\x01\x00\x02\xC0";
        assert_eq!(to_slice(&mut buf, &test).unwrap(), expected);
        let expected = b"\x93\xC3\x00\xC0";
        assert_eq!(to_slice_compact(&mut buf, &test).unwrap(), expected);

        let test = [Flavors::Strawberry, Flavors::Vanilla, Flavors::Chocolate];
        let expected = b"\x93\xAAStrawberry\xA7Vanilla\xA9Chocolate";
        assert_eq!(to_slice_named(&mut buf, &test).unwrap(), expected);
        let expected = b"\x93\x02\x00\x01";
        assert_eq!(to_slice(&mut buf, &test).unwrap(), expected);
        assert_eq!(to_slice_compact(&mut buf, &test).unwrap(), expected);

        let test = (Prices::Strawberry { gold: 7, silver: 1000 },
                    Prices::Vanilla(12.5),
                    Prices::Chocolate("free"));
        let expected = b"\x93\x81\xAAStrawberry\x82\xA4gold\x07\xA6silver\xCD\x03\xE8\x81\xA7Vanilla\xCA\x41\x48\x00\x00\x81\xA9Chocolate\xA4free";
        assert_eq!(to_slice_named(&mut buf, &test).unwrap(), expected);
        let expected = b"\x93\x81\x02\x82\x00\x07\x01\xCD\x03\xE8\x81\x00\xCA\x41\x48\x00\x00\x81\x01\xA4free";
        assert_eq!(to_slice(&mut buf, &test).unwrap(), expected);
        let expected = b"\x93\x81\x02\x92\x07\xCD\x03\xE8\x81\x00\xCA\x41\x48\x00\x00\x81\x01\xA4free";
        assert_eq!(to_slice_compact(&mut buf, &test).unwrap(), expected);
    }

    macro_rules! test_msgpack_fixint {
        ($ty:ty, $buf:ident) => {
            let int: $ty = 0;
            let expected = b"\x00";
            assert_eq!(to_slice(&mut $buf, &int).unwrap(), expected);
            assert_eq!(to_slice_named(&mut $buf, &int).unwrap(), expected);
            assert_eq!(to_slice_compact(&mut $buf, &int).unwrap(), expected);
            let int: $ty = 127;
            let expected = b"\x7f";
            assert_eq!(to_slice(&mut $buf, &int).unwrap(), expected);
            assert_eq!(to_slice_named(&mut $buf, &int).unwrap(), expected);
            assert_eq!(to_slice_compact(&mut $buf, &int).unwrap(), expected);
        };
        (- $ty:ty, $buf:ident) => {
            let int: $ty = -1;
            let expected = b"\xff";
            assert_eq!(to_slice(&mut $buf, &int).unwrap(), expected);
            assert_eq!(to_slice_named(&mut $buf, &int).unwrap(), expected);
            assert_eq!(to_slice_compact(&mut $buf, &int).unwrap(), expected);
            let int: $ty = -32;
            let expected = b"\xe0";
            assert_eq!(to_slice(&mut $buf, &int).unwrap(), expected);
            assert_eq!(to_slice_named(&mut $buf, &int).unwrap(), expected);
            assert_eq!(to_slice_compact(&mut $buf, &int).unwrap(), expected);
        };
    }

    macro_rules! test_msgpack_int {
        ($ty:ty, $buf:ident, $(($val:expr)=$exp:literal),*) => {$(
            let int: $ty = $val;
            let expected = $exp;
            assert_eq!(to_slice(&mut $buf, &int).unwrap(), expected);
            assert_eq!(to_slice_named(&mut $buf, &int).unwrap(), expected);
            assert_eq!(to_slice_compact(&mut $buf, &int).unwrap(), expected);
        )*};
    }

    #[test]
    fn test_msgpack_ints() {
        let mut buf = [0u8;9];
        test_msgpack_fixint!(i8, buf);
        test_msgpack_fixint!(u8, buf);
        test_msgpack_fixint!(- i8, buf);
        test_msgpack_fixint!(u16, buf);
        test_msgpack_fixint!(i16, buf);
        test_msgpack_fixint!(- i16, buf);
        test_msgpack_fixint!(u32, buf);
        test_msgpack_fixint!(i32, buf);
        test_msgpack_fixint!(- i32, buf);
        test_msgpack_fixint!(u64, buf);
        test_msgpack_fixint!(- i64, buf);

        test_msgpack_int!(i8, buf, (-33)=b"\xD0\xdf", (-128)=b"\xD0\x80");
        test_msgpack_int!(u8, buf, (128)=b"\xCC\x80", ( 255)=b"\xCC\xff");
        test_msgpack_int!(i16, buf, 
            (-33)=b"\xD0\xdf", (-128)=b"\xD0\x80",
            (128)=b"\xCC\x80", ( 255)=b"\xCC\xff",
            (256)=b"\xD1\x01\x00",
            (i16::MAX)=b"\xD1\x7f\xff",
            (i16::MIN)=b"\xD1\x80\x00");
        test_msgpack_int!(u16, buf, 
            (128)=b"\xCC\x80", ( 255)=b"\xCC\xff",
            (256)=b"\xCD\x01\x00",
            (u16::MAX)=b"\xCD\xff\xff");
        test_msgpack_int!(i32, buf,
            (-33)=b"\xD0\xdf", (-128)=b"\xD0\x80",
            (128)=b"\xCC\x80", ( 255)=b"\xCC\xff",
            (256)=b"\xD1\x01\x00",
            (i16::MAX.into())=b"\xD1\x7f\xff",
            (i16::MIN.into())=b"\xD1\x80\x00",
            (u16::MAX.into())=b"\xCD\xff\xff",
            (i32::MAX.into())=b"\xD2\x7f\xff\xff\xff",
            (i32::MIN.into())=b"\xD2\x80\x00\x00\x00");
        test_msgpack_int!(u32, buf, 
            (128)=b"\xCC\x80", ( 255)=b"\xCC\xff",
            (256)=b"\xCD\x01\x00",
            (u16::MAX.into())=b"\xCD\xff\xff",
            (u32::MAX)=b"\xCE\xff\xff\xff\xff");
        test_msgpack_int!(i64, buf,
            (-33)=b"\xD0\xdf", (-128)=b"\xD0\x80",
            (128)=b"\xCC\x80", ( 255)=b"\xCC\xff",
            (256)=b"\xD1\x01\x00",
            (i16::MAX.into())=b"\xD1\x7f\xff",
            (i16::MIN.into())=b"\xD1\x80\x00",
            (u16::MAX.into())=b"\xCD\xff\xff",
            (i32::MAX.into())=b"\xD2\x7f\xff\xff\xff",
            (i32::MIN.into())=b"\xD2\x80\x00\x00\x00",
            (u32::MAX.into())=b"\xCE\xff\xff\xff\xff",
            (i64::MAX.into())=b"\xD3\x7f\xff\xff\xff\xff\xff\xff\xff",
            (i64::MIN.into())=b"\xD3\x80\x00\x00\x00\x00\x00\x00\x00");
        test_msgpack_int!(u64, buf, 
            (128)=b"\xCC\x80", ( 255)=b"\xCC\xff",
            (256)=b"\xCD\x01\x00",
            (u16::MAX.into())=b"\xCD\xff\xff",
            (u32::MAX.into())=b"\xCE\xff\xff\xff\xff",
            (u64::MAX)=b"\xCF\xff\xff\xff\xff\xff\xff\xff\xff");
    }

    #[test]
    fn test_msgpack_floats() {
        let mut buf = [0u8;9];
        let flt = 0.0f32;
        let expected = b"\xCA\x00\x00\x00\x00";
        assert_eq!(to_slice(&mut buf, &flt).unwrap(), expected);
        assert_eq!(to_slice_named(&mut buf, &flt).unwrap(), expected);
        assert_eq!(to_slice_compact(&mut buf, &flt).unwrap(), expected);
        let flt = 0.0f64;
        let expected = b"\xCB\x00\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(to_slice(&mut buf, &flt).unwrap(), expected);
        assert_eq!(to_slice_named(&mut buf, &flt).unwrap(), expected);
        assert_eq!(to_slice_compact(&mut buf, &flt).unwrap(), expected);
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn test_msgpack_bytes_owned() {
        #[derive(Serialize)]
        struct Test {
            #[serde(with = "serde_bytes")]
            key: Vec<u8>
        }
        let vec = vec![169u8;65535];
        let value = [Test { key: vec }];
        let res = to_vec_named(&value).unwrap();
        assert_eq!(res.len(), 9+65535);
        assert!(res.starts_with(b"\x91\x81\xA3key\xC5\xff\xff"));
        for i in 0..65535 {
            assert_eq!(res[i+9], 169);
        }
        let res = to_vec(&value).unwrap();
        assert_eq!(res.len(), 6+65535);
        assert!(res.starts_with(b"\x91\x81\x00\xC5\xff\xff"));
        for i in 0..65535 {
            assert_eq!(res[i+6], 169);
        }
        let res = to_vec_compact(&value).unwrap();
        assert_eq!(res.len(), 5+65535);
        assert!(res.starts_with(b"\x91\x91\xC5\xff\xff"));
        for i in 0..65535 {
            assert_eq!(res[i+5], 169);
        }
    }

    #[test]
    fn test_msgpack_bytes() {
        #[derive(Serialize)]
        struct Test<'a> {
            #[serde(with = "serde_bytes")]
            key: &'a[u8]
        }
        let mut buf = [0u8;73];
        let value = [Test { key: b"\xc1\x00\x00bytes\xff" }];
        assert_eq!(to_slice_named(&mut buf, &value).unwrap(),
            b"\x91\x81\xA3key\xC4\x09\xc1\x00\x00bytes\xff");
        assert_eq!(to_slice(&mut buf, &value).unwrap(),
            b"\x91\x81\x00\xC4\x09\xc1\x00\x00bytes\xff");
        assert_eq!(to_slice_compact(&mut buf, &value).unwrap(),
            b"\x91\x91\xC4\x09\xc1\x00\x00bytes\xff");
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn test_msgpack_map() {
        let test_map = |amap, header: &[u8]| {
            let res = to_vec(&amap).unwrap();
            assert_eq!(&res[0..header.len()], header);
            let (b, len): (BTreeMap::<u32,bool>, _) = crate::from_slice(&res).unwrap();
            assert_eq!(len, res.len());
            assert_eq!(amap, b);
            assert_eq!(to_vec_compact(&amap).unwrap(), res);
            assert_eq!(to_vec_named(&amap).unwrap(), res);
        };
        let mut a = BTreeMap::<u32,bool>::new();
        for k in 0..65536 {
            a.insert(k, true);
        }
        let expected = &[0xDF, 0x00, 0x01, 0x00, 0x00];
        test_map(a, expected);

        let mut a = BTreeMap::<u32,bool>::new();
        for k in 0..256 {
            a.insert(k, true);
        }
        let expected = &[0xDE, 0x01, 0x00];
        test_map(a, expected);
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn test_msgpack_array() {
        let mut a = Vec::<i32>::new();
        for _ in 0..65536 {
            a.push(-1i32);
        }
        let mut expected = vec![0xDD, 0x00, 0x01, 0x00, 0x00];
        for _ in 0..65536 {
            expected.push(0xff);
        }
        assert_eq!(to_vec(&a).unwrap(), expected);
        assert_eq!(to_vec_compact(&a).unwrap(), expected);
        assert_eq!(to_vec_named(&a).unwrap(), expected);
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn test_msgpack_str() {
        let s = include_str!("../LICENSE-MIT");
        let mut expected = vec![0xDA];
        expected.extend_from_slice(&u16::try_from(s.len()).unwrap().to_be_bytes());
        expected.extend_from_slice(s.as_bytes());
        assert_eq!(to_vec(s).unwrap(), expected);
        assert_eq!(to_vec_compact(s).unwrap(), expected);
        assert_eq!(to_vec_named(s).unwrap(), expected);

        let mut s = String::new();
        for _ in 0..256u16 {
            for i in 0..=255u8 {
                s.push(i.into());
            }
        }
        let mut expected = vec![0xDB];
        expected.extend_from_slice(&u32::try_from(s.len()).unwrap().to_be_bytes());
        expected.extend_from_slice(s.as_bytes());
        assert_eq!(to_vec(&s).unwrap(), expected);
        assert_eq!(to_vec_compact(&s).unwrap(), expected);
        assert_eq!(to_vec_named(&s).unwrap(), expected);
    }

    #[test]
    fn test_ser_bool() {
        let mut buf = [0u8;1];
        assert_eq!(to_slice(&mut buf, &true).unwrap(), b"\xC3");
        assert_eq!(to_slice_compact(&mut buf, &true).unwrap(), b"\xC3");
        assert_eq!(to_slice_named(&mut buf, &true).unwrap(), b"\xC3");
        assert_eq!(to_slice(&mut buf, &false).unwrap(), b"\xC2");
        assert_eq!(to_slice_compact(&mut buf, &false).unwrap(), b"\xC2");
        assert_eq!(to_slice_named(&mut buf, &false).unwrap(), b"\xC2");
    }

    #[test]
    fn test_ser_str() {
        let mut buf = [0u8;256];
        assert_eq!(to_slice(&mut buf, "hello").unwrap(), b"\xA5hello");
        assert_eq!(to_slice_compact(&mut buf, "hello").unwrap(), b"\xA5hello");
        assert_eq!(to_slice_named(&mut buf, "hello").unwrap(), b"\xA5hello");
        assert_eq!(to_slice(&mut buf, "").unwrap(), b"\xA0");
        assert_eq!(to_slice_compact(&mut buf, "").unwrap(), b"\xA0");
        assert_eq!(to_slice_named(&mut buf, "").unwrap(), b"\xA0");

        assert_eq!(to_slice(&mut buf, "ä").unwrap(), b"\xA2\xC3\xA4");
        assert_eq!(to_slice_compact(&mut buf, "ä").unwrap(), b"\xA2\xC3\xA4");
        assert_eq!(to_slice_named(&mut buf, "ä").unwrap(), b"\xA2\xC3\xA4");
        assert_eq!(to_slice(&mut buf, "৬").unwrap(), b"\xA3\xe0\xa7\xac");
        assert_eq!(to_slice_compact(&mut buf, "৬").unwrap(), b"\xA3\xe0\xa7\xac");
        assert_eq!(to_slice_named(&mut buf, "৬").unwrap(), b"\xA3\xe0\xa7\xac");
        assert_eq!(to_slice(&mut buf, "\u{A0}").unwrap(), b"\xA2\xC2\xA0"); // non-breaking space
        assert_eq!(to_slice_compact(&mut buf, "\u{A0}").unwrap(), b"\xA2\xC2\xA0"); // non-breaking space
        assert_eq!(to_slice_named(&mut buf, "\u{A0}").unwrap(), b"\xA2\xC2\xA0"); // non-breaking space
        assert_eq!(to_slice(&mut buf, "ℝ").unwrap(), b"\xA3\xe2\x84\x9d"); // 3 byte character
        assert_eq!(to_slice(&mut buf, "💣").unwrap(), b"\xA4\xf0\x9f\x92\xa3"); // 4 byte character
        assert_eq!(to_slice_compact(&mut buf, "💣").unwrap(), b"\xA4\xf0\x9f\x92\xa3"); // 4 byte character
        assert_eq!(to_slice_named(&mut buf, "💣").unwrap(), b"\xA4\xf0\x9f\x92\xa3"); // 4 byte character

        assert_eq!(to_slice(&mut buf, "\r").unwrap(), b"\xA1\r");
        assert_eq!(to_slice_compact(&mut buf, "\x00\t\r\n").unwrap(), b"\xA4\x00\t\r\n");
        assert_eq!(to_slice_named(&mut buf, "\x00\t\r\n").unwrap(), b"\xA4\x00\t\r\n");

        let s = "Γαζίες καὶ μυρτιὲς δὲν θὰ βρῶ πιὰ στὸ χρυσαφὶ ξέφωτο";
        let expected = b"\xD9\x67\xce\x93\xce\xb1\xce\xb6\xce\xaf\xce\xb5\xcf\x82\x20\xce\xba\xce\xb1\xe1\xbd\xb6\x20\xce\xbc\xcf\x85\xcf\x81\xcf\x84\xce\xb9\xe1\xbd\xb2\xcf\x82\x20\xce\xb4\xe1\xbd\xb2\xce\xbd\x20\xce\xb8\xe1\xbd\xb0\x20\xce\xb2\xcf\x81\xe1\xbf\xb6\x20\xcf\x80\xce\xb9\xe1\xbd\xb0\x20\xcf\x83\xcf\x84\xe1\xbd\xb8\x20\xcf\x87\xcf\x81\xcf\x85\xcf\x83\xce\xb1\xcf\x86\xe1\xbd\xb6\x20\xce\xbe\xce\xad\xcf\x86\xcf\x89\xcf\x84\xce\xbf";
        assert_eq!(to_slice(&mut buf, s).unwrap(), expected);
        assert_eq!(to_slice_compact(&mut buf, s).unwrap(), expected);
        assert_eq!(to_slice_named(&mut buf, s).unwrap(), expected);
    }

    #[test]
    fn test_ser_array() {
        let mut buf = [0u8;19];
        let empty: [&str;0] = [];
        assert_eq!(to_slice(&mut buf, &empty).unwrap(), b"\x90");
        assert_eq!(to_slice_compact(&mut buf, &empty).unwrap(), b"\x90");
        assert_eq!(to_slice_named(&mut buf, &empty).unwrap(), b"\x90");
        assert_eq!(to_slice(&mut buf, &[0, 1, 2]).unwrap(), b"\x93\x00\x01\x02");
        assert_eq!(to_slice_compact(&mut buf, &[0, 1, 2]).unwrap(), b"\x93\x00\x01\x02");
        assert_eq!(to_slice_named(&mut buf, &[0, 1, 2]).unwrap(), b"\x93\x00\x01\x02");
        let ary = [-1i8;15];
        let expected = b"\x9F\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF";
        assert_eq!(to_slice(&mut buf, &ary).unwrap(), expected);
        assert_eq!(to_slice_compact(&mut buf, &ary).unwrap(), expected);
        assert_eq!(to_slice_named(&mut buf, &ary).unwrap(), expected);
        let ary = [-1i32;16];
        let expected = b"\xDC\x00\x10\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF";
        assert_eq!(to_slice(&mut buf, &ary).unwrap(), expected);
        assert_eq!(to_slice_compact(&mut buf, &ary).unwrap(), expected);
        assert_eq!(to_slice_named(&mut buf, &ary).unwrap(), expected);
    }

    #[test]
    fn test_ser_enum() {
        #[derive(Serialize)]
        enum Type {
            #[serde(rename = "boolean")]
            Boolean,
            #[serde(rename = "number")]
            Number,
        }
        let mut buf = [0u8;8];

        assert_eq!(
            to_slice(&mut buf, &Type::Boolean).unwrap(),
            b"\x00");
        assert_eq!(
            to_slice_compact(&mut buf, &Type::Boolean).unwrap(),
            b"\x00");
        assert_eq!(
            to_slice_named(&mut buf, &Type::Boolean).unwrap(),
            b"\xA7boolean");

        assert_eq!(
            to_slice(&mut buf, &Type::Number).unwrap(),
            b"\x01");
        assert_eq!(
            to_slice_compact(&mut buf, &Type::Number).unwrap(),
            b"\x01");
        assert_eq!(
            to_slice_named(&mut buf, &Type::Number).unwrap(),
            b"\xA6number");
    }

    #[test]
    fn test_ser_struct_bool() {
        #[derive(Serialize)]
        struct Led {
            led: bool,
        }

        let mut buf = [0u8;6];

        assert_eq!(
            to_slice_compact(&mut buf, &Led { led: true }).unwrap(),
            b"\x91\xC3");
        assert_eq!(
            to_slice(&mut buf, &Led { led: true }).unwrap(),
            b"\x81\x00\xC3");
        assert_eq!(
            to_slice_named(&mut buf, &Led { led: true }).unwrap(),
            b"\x81\xA3led\xC3");
    }

    #[test]
    fn test_ser_struct_i8() {
        #[derive(Serialize)]
        struct Temperature {
            temperature: i8,
        }

        let mut buf = [0u8;15];

        assert_eq!(
            to_slice_compact(&mut buf, &Temperature { temperature: 127 }).unwrap(),
            b"\x91\x7f");
        assert_eq!(
            to_slice(&mut buf, &Temperature { temperature: 127 }).unwrap(),
            b"\x81\x00\x7f");
        assert_eq!(
            to_slice_named(&mut buf, &Temperature { temperature: 127 }).unwrap(),
            b"\x81\xABtemperature\x7f");

        assert_eq!(
            to_slice_compact(&mut buf, &Temperature { temperature: 20 }).unwrap(),
            b"\x91\x14");
        assert_eq!(
            to_slice(&mut buf, &Temperature { temperature: 20 }).unwrap(),
            b"\x81\x00\x14");
        assert_eq!(
            to_slice_named(&mut buf, &Temperature { temperature: 20 }).unwrap(),
            b"\x81\xABtemperature\x14");

        assert_eq!(
            to_slice_compact(&mut buf, &Temperature { temperature: -17 }).unwrap(),
            b"\x91\xef");
        assert_eq!(
            to_slice(&mut buf, &Temperature { temperature: -17 }).unwrap(),
            b"\x81\x00\xef");
        assert_eq!(
            to_slice_named(&mut buf, &Temperature { temperature: -17 }).unwrap(),
            b"\x81\xABtemperature\xef");

        assert_eq!(
            to_slice_compact(&mut buf, &Temperature { temperature: -128 }).unwrap(),
            b"\x91\xD0\x80");
        assert_eq!(
            to_slice(&mut buf, &Temperature { temperature: -128 }).unwrap(),
            b"\x81\x00\xD0\x80");
        assert_eq!(
            to_slice_named(&mut buf, &Temperature { temperature: -128 }).unwrap(),
            b"\x81\xABtemperature\xD0\x80");
    }

    #[test]
    fn test_ser_struct_u8() {
        #[derive(Serialize)]
        struct Temperature {
            temperature: u8,
        }

        let mut buf = [0u8;15];

        assert_eq!(
            to_slice_compact(&mut buf, &Temperature { temperature: 127 }).unwrap(),
            b"\x91\x7f");
        assert_eq!(
            to_slice(&mut buf, &Temperature { temperature: 127 }).unwrap(),
            b"\x81\x00\x7f");
        assert_eq!(
            to_slice_named(&mut buf, &Temperature { temperature: 127 }).unwrap(),
            b"\x81\xABtemperature\x7f"
        );

        assert_eq!(
            to_slice_compact(&mut buf, &Temperature { temperature: 128 }).unwrap(),
            b"\x91\xCC\x80");
        assert_eq!(
            to_slice(&mut buf, &Temperature { temperature: 128 }).unwrap(),
            b"\x81\x00\xCC\x80");
        assert_eq!(
            to_slice_named(&mut buf, &Temperature { temperature: 128 }).unwrap(),
            b"\x81\xABtemperature\xCC\x80");
    }

    #[test]
    fn test_ser_struct_f32() {
        #[derive(Serialize)]
        struct Temperature {
            temperature: f32,
        }

        let mut buf = [0u8;18];

        assert_eq!(
            to_slice_compact(&mut buf, &Temperature { temperature: -20.0 }).unwrap(),
            b"\x91\xCA\xc1\xa0\x00\x00");
        assert_eq!(
            to_slice(&mut buf, &Temperature { temperature: -20.0 }).unwrap(),
            b"\x81\x00\xCA\xc1\xa0\x00\x00");
        assert_eq!(
            to_slice_named(&mut buf, &Temperature { temperature: -20.0 }).unwrap(),
            b"\x81\xABtemperature\xCA\xc1\xa0\x00\x00");

        let temp = Temperature {
            temperature: -2.3456789012345e-23
        };
        assert_eq!(
            to_slice_compact(&mut buf, &temp).unwrap(),
            b"\x91\xCA\x99\xe2\xdc\x32");
        assert_eq!(
            to_slice(&mut buf, &temp).unwrap(),
            b"\x81\x00\xCA\x99\xe2\xdc\x32");
        assert_eq!(
            to_slice_named(&mut buf, &temp).unwrap(),
            b"\x81\xABtemperature\xCA\x99\xe2\xdc\x32");

        let temp = Temperature {
            temperature: f32::NAN
        };
        assert_eq!(
            to_slice_compact(&mut buf, &temp).unwrap(),
            b"\x91\xCA\x7f\xc0\x00\x00");
        assert_eq!(
            to_slice(&mut buf, &temp).unwrap(),
            b"\x81\x00\xCA\x7f\xc0\x00\x00");
        assert_eq!(
            to_slice_named(&mut buf, &temp).unwrap(),
            b"\x81\xABtemperature\xCA\x7f\xc0\x00\x00");

        let temp = Temperature {
            temperature: f32::NEG_INFINITY
        };
        assert_eq!(
            to_slice_compact(&mut buf, &temp).unwrap(),
            b"\x91\xCA\xff\x80\x00\x00");
        assert_eq!(
            to_slice(&mut buf, &temp).unwrap(),
            b"\x81\x00\xCA\xff\x80\x00\x00");
        assert_eq!(
            to_slice_named(&mut buf, &temp).unwrap(),
            b"\x81\xABtemperature\xCA\xff\x80\x00\x00");
    }

    #[test]
    fn test_ser_struct_option() {
        #[derive(Serialize)]
        struct Property<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            description: Option<&'a str>,
            value: Option<u32>,
        }
        #[derive(Serialize)]
        struct Skippable<'a> {
            value: Option<u32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            description: Option<&'a str>
        }

        let mut buf = [0u8;54];

        assert_eq!(
            to_slice_compact(&mut buf, &Property {
                description: Some("An ambient temperature sensor"), value: None,
            })
            .unwrap(),
            b"\x92\xBDAn ambient temperature sensor\xC0");
        assert_eq!(
            to_slice(&mut buf, &Property {
                description: Some("An ambient temperature sensor"), value: None,
            })
            .unwrap(),
            b"\x82\x00\xBDAn ambient temperature sensor\x01\xC0");
        assert_eq!(
            to_slice_named(&mut buf, &Property {
                description: Some("An ambient temperature sensor"), value: None,
            })
            .unwrap(),
            b"\x82\xABdescription\xBDAn ambient temperature sensor\xA5value\xC0");

        let property = Property { description: None, value: None };
        assert_eq!(
            to_slice_compact(&mut buf, &property),
            Err(Error::FieldSkipped));
        assert_eq!(
            to_slice(&mut buf, &property).unwrap(),
            b"\x81\x01\xC0");
        assert_eq!(
            to_slice_named(&mut buf, &property).unwrap(),
            b"\x81\xA5value\xC0");

        let property = Property { description: None, value: Some(0) };
        assert_eq!(
            to_slice_compact(&mut buf, &property),
            Err(Error::FieldSkipped));
        assert_eq!(
            to_slice(&mut buf, &property).unwrap(),
            b"\x81\x01\x00");
        assert_eq!(
            to_slice_named(&mut buf, &property).unwrap(),
            b"\x81\xA5value\x00");

        let property = Property {
            description: Some("Answer to the Ultimate Question?"),
            value: Some(42)
        };
        assert_eq!(
            to_slice_compact(&mut buf, &property).unwrap(),
            b"\x92\xD9\x20Answer to the Ultimate Question?\x2A");
        assert_eq!(
            to_slice(&mut buf, &property).unwrap(),
            b"\x82\x00\xD9\x20Answer to the Ultimate Question?\x01\x2A");
        assert_eq!(
            to_slice_named(&mut buf, &property).unwrap(),
            b"\x82\xABdescription\xD9\x20Answer to the Ultimate Question?\xA5value\x2A");

        let skippable = Skippable { value: None, description: None};
        assert_eq!(
            to_slice_compact(&mut buf, &skippable).unwrap(),
            b"\x91\xC0");
        assert_eq!(
            to_slice(&mut buf, &skippable).unwrap(),
            b"\x81\x00\xC0");
        assert_eq!(
            to_slice_named(&mut buf, &skippable).unwrap(),
            b"\x81\xA5value\xC0");

        let skippable = Skippable { value: Some(0), description: None};
        assert_eq!(
            to_slice_compact(&mut buf, &skippable).unwrap(),
            b"\x91\x00");
        assert_eq!(
            to_slice(&mut buf, &skippable).unwrap(),
            b"\x81\x00\x00");
        assert_eq!(
            to_slice_named(&mut buf, &skippable).unwrap(),
            b"\x81\xA5value\x00");
    }


    #[test]
    fn test_ser_struct_() {
        #[derive(Serialize)]
        struct Empty {}

        let mut buf = [0u8;20];

        assert_eq!(to_slice_compact(&mut buf, &Empty {}).unwrap(), &[0x90]);
        assert_eq!(to_slice(&mut buf, &Empty {}).unwrap(), &[0x80]);
        assert_eq!(to_slice_named(&mut buf, &Empty {}).unwrap(), &[0x80]);

        #[derive(Serialize)]
        struct Tuple {
            a: bool,
            b: bool,
        }

        let tuple = Tuple { a: true, b: false };
        assert_eq!(
            to_slice_compact(&mut buf, &tuple).unwrap(),
            b"\x92\xC3\xC2");
        assert_eq!(
            to_slice(&mut buf, &tuple).unwrap(),
            b"\x82\x00\xC3\x01\xC2");
        assert_eq!(
            to_slice_named(&mut buf, &tuple).unwrap(),
            b"\x82\xA1a\xC3\xA1b\xC2");
    }

    #[test]
    fn test_ser_unit() {
        let mut buf = [0u8;1];
        let a = ();
        assert_eq!(to_slice(&mut buf, &a).unwrap(), b"\xC0");
        assert_eq!(to_slice_named(&mut buf, &a).unwrap(), b"\xC0");
        assert_eq!(to_slice_compact(&mut buf, &a).unwrap(), b"\xC0");
        #[derive(Serialize)]
        struct Unit;
        let a = Unit;
        assert_eq!(to_slice(&mut buf, &a).unwrap(), b"\xC0");
        assert_eq!(to_slice_named(&mut buf, &a).unwrap(), b"\xC0");
        assert_eq!(to_slice_compact(&mut buf, &a).unwrap(), b"\xC0");
    }

    #[test]
    fn test_ser_newtype_struct() {
        #[derive(Serialize)]
        struct A(pub u32);
        let mut buf = [0u8;1];
        let a = A(54);
        assert_eq!(to_slice(&mut buf, &a).unwrap(), &[54]);
        assert_eq!(to_slice_named(&mut buf, &a).unwrap(), &[54]);
        assert_eq!(to_slice_compact(&mut buf, &a).unwrap(), &[54]);
    }

    #[test]
    fn test_ser_newtype_variant() {
        #[derive(Serialize)]
        enum A {
            A(u32),
        }
        let mut buf = [0u8;8];

        let a = A::A(54);
        assert_eq!(to_slice(&mut buf, &a).unwrap(), &[0x81,0x00,54]);
        assert_eq!(to_slice_compact(&mut buf, &a).unwrap(), &[0x81,0x00,54]);
        assert_eq!(to_slice_named(&mut buf, &a).unwrap(), &[0x81,0xA1,b'A',54]);
    }

    #[test]
    fn test_ser_struct_variant() {
        #[derive(Serialize)]
        enum A {
            A { x: u32, y: u16 },
        }
        let mut buf = [0u8;12];
        let a = A::A { x: 54, y: 720 };

        assert_eq!(
            to_slice_compact(&mut buf, &a).unwrap(),
            &[0x81,0x00, 0x92,54, 0xCD,0x02,0xD0]);
        assert_eq!(
            to_slice(&mut buf, &a).unwrap(),
            &[0x81,0x00, 0x82,0x00,54, 0x01,0xCD,0x02,0xD0]);
        assert_eq!(
            to_slice_named(&mut buf, &a).unwrap(),
            &[0x81,0xA1,b'A', 0x82,0xA1,b'x',54, 0xA1,b'y',0xCD,0x02,0xD0]);
    }

    #[test]
    fn test_ser_tuple_struct() {
        #[derive(Serialize)]
        struct A<'a>(u32, Option<&'a str>, u16, bool);

        let mut buf = [0u8;15];
        let a = A(42, Some("A string"), 720, false);

        assert_eq!(
            to_slice_compact(&mut buf, &a).unwrap(),
            b"\x94\x2A\xA8A string\xCD\x02\xD0\xC2");
        assert_eq!(
            to_slice(&mut buf, &a).unwrap(),
            b"\x94\x2A\xA8A string\xCD\x02\xD0\xC2");
        assert_eq!(
            to_slice_named(&mut buf, &a).unwrap(),
            b"\x94\x2A\xA8A string\xCD\x02\xD0\xC2");
    }

    #[test]
    fn test_ser_tuple_struct_roundtrip() {
        use serde::Deserialize;

        #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
        struct A<'a>(u32, Option<&'a str>, u16, bool);

        let mut buf = [0u8;25];
        let a1 = A(42, Some("A string"), 720, false);

        let mut writer = SliceWriter::new(&mut buf);
        to_writer(&mut writer, &a1).unwrap();
        let mut serialized = writer.split().0;
        let (a2, len): (A<'_>, _) = crate::from_slice(&mut serialized).unwrap();
        assert_eq!(len, 15);
        assert_eq!(a1, a2);
    }

}
