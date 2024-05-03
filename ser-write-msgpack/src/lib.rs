//! MessagePack serializers for ser-write
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "std")]
extern crate std;

#[cfg(all(feature = "alloc",not(feature = "std")))]
extern crate alloc;

use core::ops::RangeInclusive;
use core::fmt;

#[cfg(feature = "std")]
use std::{vec::Vec, string::ToString};

#[cfg(all(feature = "alloc",not(feature = "std")))]
use alloc::{vec::Vec, string::ToString};

use serde::{ser, Serialize};
use ser::Serializer as _;
use ser_write::{SerResult as Result};

pub use ser_write::{SerWrite, SerError, SerResult};

pub struct Serializer<W> {
    output: W
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
/// Serialize JSON to a Vec
pub fn to_vec<T>(value: &T, vec: &mut Vec<u8>) -> Result<()>
    where T: Serialize,
{
    to_writer(vec, value)
}

/// Serialize JSON to a SerWrite implementation
pub fn to_writer<W, T>(writer: W, value: &T) -> Result<()>
    where W: SerWrite, T: Serialize
{
    let mut serializer = Serializer::new(writer);
    value.serialize(&mut serializer)
}

/* MessagePack MAGICK */
const MAX_POSFIXINT: u8 = 0x7f;
const MIN_NEGFIXINT: i8 = 0b11100000u8 as i8; //-32
const FIXINT_I16: RangeInclusive<i16> = MIN_NEGFIXINT as i16..=MAX_POSFIXINT as i16;
const FIXINT_I32: RangeInclusive<i32> = MIN_NEGFIXINT as i32..=MAX_POSFIXINT as i32;
const FIXINT_I64: RangeInclusive<i64> = MIN_NEGFIXINT as i64..=MAX_POSFIXINT as i64;
const NIL: u8      = 0xc0;
const FALSE: u8    = 0xc2;
const TRUE: u8     = 0xc3;
const FIXMAP: u8   = 0x80; /* 1000xxxx */
const MAX_FIXMAP_SIZE: usize = 0b1111;
const FIXARRAY: u8 = 0x90; /* 1001xxxx */
const MAX_FIXARRAY_SIZE: usize = 0b1111;
const FIXSTR: u8   = 0xa0; /* 101xxxxx */
const MAX_FIXSTR_SIZE: usize = 0b11111;
const BIN_8: u8    = 0xc4;
const BIN_16: u8   = 0xc5;
const BIN_32: u8   = 0xc6;
const FLOAT_32: u8 = 0xca;
const FLOAT_64: u8 = 0xcb;
const UINT_8: u8   = 0xcc;
const UINT_16: u8  = 0xcd;
const UINT_32: u8  = 0xce;
const UINT_64: u8  = 0xcf;
const INT_8: u8    = 0xd0;
const INT_16: u8   = 0xd1;
const INT_32: u8   = 0xd2;
const INT_64: u8   = 0xd3;
const STR_8: u8    = 0xd9;
const STR_16: u8   = 0xda;
const STR_32: u8   = 0xdb;
const ARRAY_16: u8 = 0xdc;
const ARRAY_32: u8 = 0xdd;
const MAP_16: u8   = 0xde;
const MAP_32: u8   = 0xdf;

impl<W> Serializer<W> {
    #[inline(always)]
    pub fn new(output: W) -> Self {
        Serializer { output }
    }

    #[inline(always)]
    pub fn into_inner(self) -> W {
        self.output
    }
}

impl<'a, W: SerWrite> ser::Serializer for &'a mut Serializer<W> {
    type Ok = ();
    type Error = SerError;

    type SerializeSeq = SerializeSeqMap<'a, W>;
    type SerializeTuple = SerializeSeqMap<'a, W>;
    type SerializeTupleStruct = SerializeSeqMap<'a, W>;
    type SerializeTupleVariant = SerializeSeqMap<'a, W>;
    type SerializeMap = SerializeSeqMap<'a, W>;
    type SerializeStruct = SerializeSeqMap<'a, W>;
    type SerializeStructVariant = SerializeSeqMap<'a, W>;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.output.write_byte(if v { TRUE } else { FALSE })
    }
    #[inline(always)]
    fn serialize_i8(self, v: i8) -> Result<()> {
        if v >= MIN_NEGFIXINT {
            self.output.write_byte(v as u8)
        }
        else {
            self.output.write_byte(INT_8)?;
            self.output.write_byte(v as u8)
        }
    }
    #[inline(always)]
    fn serialize_i16(self, v: i16) -> Result<()> {
        if FIXINT_I16.contains(&v) {
            self.output.write_byte(v as u8)
        }
        else if let Ok(v) = i8::try_from(v) {
            self.output.write_byte(INT_8)?;
            self.output.write_byte(v as u8)
        }
        else if let Ok(v) = u8::try_from(v) {
            self.output.write_byte(UINT_8)?;
            self.output.write_byte(v)            
        }
        else {
            self.output.write_byte(INT_16)?;
            self.output.write(&v.to_be_bytes())
        }
    }
    #[inline]
    fn serialize_i32(self, v: i32) -> Result<()> {
        if FIXINT_I32.contains(&v) {
            self.output.write_byte(v as u8)
        }
        else if let Ok(v) = i8::try_from(v) {
            self.output.write_byte(INT_8)?;
            self.output.write_byte(v as u8)
        }
        else if let Ok(v) = u8::try_from(v) {
            self.output.write_byte(UINT_8)?;
            self.output.write_byte(v)
        }
        else if let Ok(v) = i16::try_from(v) {
            self.output.write_byte(INT_16)?;
            self.output.write(&v.to_be_bytes())
        }
        else if let Ok(v) = u16::try_from(v) {
            self.output.write_byte(UINT_16)?;
            self.output.write(&v.to_be_bytes())
        }
        else {
            self.output.write_byte(INT_32)?;
            self.output.write(&v.to_be_bytes())
        }
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        if FIXINT_I64.contains(&v) {
            self.output.write_byte(v as u8)
        }
        else if let Ok(v) = i8::try_from(v) {
            self.output.write_byte(INT_8)?;
            self.output.write_byte(v as u8)
        }
        else if let Ok(v) = u8::try_from(v) {
            self.output.write_byte(UINT_8)?;
            self.output.write_byte(v)
        }
        else if let Ok(v) = i16::try_from(v) {
            self.output.write_byte(INT_16)?;
            self.output.write(&v.to_be_bytes())
        }
        else if let Ok(v) = u16::try_from(v) {
            self.output.write_byte(UINT_16)?;
            self.output.write(&v.to_be_bytes())
        }
        else if let Ok(v) = i32::try_from(v) {
            self.output.write_byte(INT_32)?;
            self.output.write(&v.to_be_bytes())
        }
        else if let Ok(v) = u32::try_from(v) {
            self.output.write_byte(UINT_32)?;
            self.output.write(&v.to_be_bytes())
        }
        else {
            self.output.write_byte(INT_64)?;
            self.output.write(&v.to_be_bytes())
        }
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        if v <= MAX_POSFIXINT {
            self.output.write_byte(v)
        }
        else {
            self.output.write_byte(UINT_8)?;
            self.output.write_byte(v)
        }
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        if v <= MAX_POSFIXINT as u16 {
            self.output.write_byte(v as u8)
        }
        else if let Ok(v) = u8::try_from(v) {
            self.output.write_byte(UINT_8)?;
            self.output.write_byte(v)
        }
        else {
            self.output.write_byte(UINT_16)?;
            self.output.write(&v.to_be_bytes())
        }
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok> {
        if v <= MAX_POSFIXINT as u32 {
            self.output.write_byte(v as u8)
        }
        else if let Ok(v) = u8::try_from(v) {
            self.output.write_byte(UINT_8)?;
            self.output.write_byte(v)
        }
        else if let Ok(v) = u16::try_from(v) {
            self.output.write_byte(UINT_16)?;
            self.output.write(&v.to_be_bytes())
        }
        else {
            self.output.write_byte(UINT_32)?;
            self.output.write(&v.to_be_bytes())
        }
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok> {
        if v <= MAX_POSFIXINT as u64 {
            self.output.write_byte(v as u8)
        }
        else if let Ok(v) = u8::try_from(v) {
            self.output.write_byte(UINT_8)?;
            self.output.write_byte(v)
        }
        else if let Ok(v) = u16::try_from(v) {
            self.output.write_byte(UINT_16)?;
            self.output.write(&v.to_be_bytes())
        }
        else if let Ok(v) = u32::try_from(v) {
            self.output.write_byte(UINT_32)?;
            self.output.write(&v.to_be_bytes())
        }
        else {
            self.output.write_byte(UINT_64)?;
            self.output.write(&v.to_be_bytes())
        }
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        self.output.write_byte(FLOAT_32)?;
        self.output.write(&v.to_be_bytes())
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        self.output.write_byte(FLOAT_64)?;
        self.output.write(&v.to_be_bytes())
    }

    fn serialize_char(self, v: char) -> Result<()> {
        let mut encoding_tmp = [0u8; 4];
        let encoded = v.encode_utf8(&mut encoding_tmp);
        self.serialize_str(encoded)
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        let size = v.len();
        write_str_len(&mut self.output, size)?;
        self.output.write_str(v)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
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
            return Err(SerError::BufferFull)
        }
        self.output.write(v)
    }

    fn serialize_none(self) -> Result<()> {
        self.output.write_byte(NIL)
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
        where T: ?Sized + Serialize
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        self.serialize_none()
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<()>
        where T: ?Sized + Serialize
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.output.write_byte(FIXMAP|1)?;
        self.serialize_str(variant)?; /* use variant_index? */
        value.serialize(&mut *self)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        let len = len.ok_or(SerError::SeqLength)?;
        if len <= MAX_FIXARRAY_SIZE {
            self.output.write_byte(FIXARRAY | (len as u8))?;
        }
        else if let Ok(len) = u16::try_from(len) {
            self.output.write_byte(ARRAY_16)?;
            self.output.write(&len.to_be_bytes())?;
        }
        else if let Ok(len) = u32::try_from(len) {
            self.output.write_byte(ARRAY_32)?;
            self.output.write(&len.to_be_bytes())?;
        }
        else {
            return Err(SerError::BufferFull)
        }
        Ok(SerializeSeqMap::new(len, self))
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_seq(Some(len))
    }

    // Tuple variants are represented in JSON as `{ NAME: [ ... ] }`.
    // This is the externally tagged representation.
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.output.write_byte(FIXMAP|1)?;
        self.serialize_str(variant)?; /* use variant_index? */
        self.serialize_seq(Some(len))
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        let len = len.ok_or(SerError::SeqLength)?;
        if len <= MAX_FIXMAP_SIZE {
            self.output.write_byte(FIXMAP | (len as u8))?;
        }
        else if let Ok(len) = u16::try_from(len) {
            self.output.write_byte(MAP_16)?;
            self.output.write(&len.to_be_bytes())?;
        }
        else if let Ok(len) = u32::try_from(len) {
            self.output.write_byte(MAP_32)?;
            self.output.write(&len.to_be_bytes())?;
        }
        else {
            return Err(SerError::BufferFull)
        }
        Ok(SerializeSeqMap::new(len, self))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct> {
        self.serialize_map(Some(len))
    }

    // Struct variants are represented in JSON as `{ NAME: { K: V, ... } }`.
    // This is the externally tagged representation.
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.output.write_byte(FIXMAP|1)?;
        self.serialize_str(variant)?; /* use variant_index? */
        self.serialize_map(Some(len))
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    fn collect_str<T: ?Sized>(self, value: &T) -> Result<Self::Ok>
        where T: fmt::Display
    {
        self.serialize_str(&value.to_string())
    }

    #[cfg(not(any(feature = "std", feature = "alloc")))]
    /// This implementation will format the value string twice, once to establish its size and later to actually
    /// write the string.
    fn collect_str<T: ?Sized>(self, value: &T) -> Result<Self::Ok>
        where T: fmt::Display
    {
        if let Some(s) = format_args!("{}", value).as_str() {
            return self.serialize_str(s)
        }
        let mut col = StringLenCounter(0);
        fmt::write(&mut col, format_args!("{}", value)).map_err(|_| SerError::BufferFull)?;
        let StringLenCounter(len) = col;
        write_str_len(&mut self.output, len)?;
        let mut col = StringCollector::new(len, &mut self.output);
        fmt::write(&mut col, format_args!("{}", value)).map_err(|_| SerError::BufferFull)
    }
}

#[inline]
fn write_str_len<W: SerWrite>(output: &mut W, len: usize) -> Result<()> {
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
        return Err(SerError::BufferFull)
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

pub struct SerializeSeqMap<'a, W> {
    len: usize,
    ser: &'a mut Serializer<W>
}

impl<'a, W: SerWrite> SerializeSeqMap<'a, W> {
    fn new(len: usize, ser: &'a mut Serializer<W>) -> Self {
        SerializeSeqMap { len, ser }
    }
}

// This impl is SerializeSeq so these methods are called after `serialize_seq`
// is called on the Serializer.
impl<'a, W: SerWrite> ser::SerializeSeq for SerializeSeqMap<'a, W> {
    type Ok = ();
    type Error = SerError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
        where T: ?Sized + Serialize
    {
        self.len = self.len.checked_sub(1).ok_or(SerError::SeqLength)?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        (self.len == 0).then_some(()).ok_or(SerError::SeqLength)
    }
}

impl<'a, W: SerWrite> ser::SerializeTuple for SerializeSeqMap<'a, W> {
    type Ok = ();
    type Error = SerError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where T: ?Sized + Serialize
    {
        self.len = self.len.checked_sub(1).ok_or(SerError::SeqLength)?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        (self.len == 0).then_some(()).ok_or(SerError::SeqLength)
    }
}

impl<'a, W: SerWrite> ser::SerializeTupleStruct for SerializeSeqMap<'a, W> {
    type Ok = ();
    type Error = SerError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
        where T: ?Sized + Serialize
    {
        self.len = self.len.checked_sub(1).ok_or(SerError::SeqLength)?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        (self.len == 0).then_some(()).ok_or(SerError::SeqLength)
    }
}

// Tuple variants are a little different. { NAME: [ ... ]}
impl<'a, W: SerWrite> ser::SerializeTupleVariant for SerializeSeqMap<'a, W> {
    type Ok = ();
    type Error = SerError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where T: ?Sized + Serialize
    {
        self.len = self.len.checked_sub(1).ok_or(SerError::SeqLength)?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        (self.len == 0).then_some(()).ok_or(SerError::SeqLength)
    }
}

impl<'a, W: SerWrite> ser::SerializeMap for SerializeSeqMap<'a, W> {
    type Ok = ();
    type Error = SerError;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
        where T: ?Sized + Serialize
    {
        self.len = self.len.checked_sub(1).ok_or(SerError::MapLength)?;
        key.serialize(&mut *self.ser)
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where T: ?Sized + Serialize
    {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        (self.len == 0).then_some(()).ok_or(SerError::MapLength)
    }
}

impl<'a, W: SerWrite> ser::SerializeStruct for SerializeSeqMap<'a, W> {
    type Ok = ();
    type Error = SerError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
        where T: ?Sized + Serialize
    {
        self.len = self.len.checked_sub(1).ok_or(SerError::MapLength)?;
        self.ser.serialize_str(key)?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        (self.len == 0).then_some(()).ok_or(SerError::MapLength)
    }
}

impl<'a, W: SerWrite> ser::SerializeStructVariant for SerializeSeqMap<'a, W> {
    type Ok = ();
    type Error = SerError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
        where T: ?Sized + Serialize
    {
        self.len = self.len.checked_sub(1).ok_or(SerError::MapLength)?;
        self.ser.serialize_str(key)?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        (self.len == 0).then_some(()).ok_or(SerError::MapLength)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msgpack() {

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
        to_vec(&test, &mut vec).unwrap();
        assert_eq!(&vec, expected);
    }
}
