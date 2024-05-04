use core::fmt;

#[cfg(feature = "std")]
use std::{vec::Vec, string::ToString};

#[cfg(all(feature = "alloc",not(feature = "std")))]
use alloc::{vec::Vec, string::ToString};

use serde::{ser, Serialize};
use ser::Serializer as _;
use ser_write::{SerResult as Result};
use super::magick::*;

pub use ser_write::{SerWrite, SerError, SerResult};

/// MessagePack serializer serializing structs to arrays and enum variants as indexes
pub type StructCompactSerializer<W> = Serializer<W>;
/// MessagePack serializer serializing structs to maps with field names and enum variants as names
// pub type StructFieldNameSerializer<W> = Serializer<W, StructFieldNameFormatter>;

pub struct Serializer<W> {
    output: W
}

// impl<'a, W: SerWrite + 'a, F: VariantFormat + 'a> StructFormat<'a, W, F> for StructCompactFormatter {
//     type SerializeStruct = SerializeStructArray<'a, Welf>;
//     fn serialize_struct(ser: &'a mut Serializer<Welf>, len: usize) -> Result<Self::SerializeStruct>
//         where &'a mut Serializer<Welf>: ser::Serializer<Ok=(), Error=SerError>
//     {
//         SerializeStructArray::start(ser, len)
//     }
// }

/// Implements [`VariantFormat`] serializing structs and tuple structs to arrays and enum variants as indexes
pub struct StructCompactFormatter;
/// Implements [`VariantFormat`] serializing structs and tuple structs to maps and enum variants as names
// pub struct StructFieldNameFormatter;

// impl<W: SerWrite> VariantFormat<W> for StructFieldNameFormatter {
//     type SerializeStruct = SerializeStruct<'a, W, Self>;
//     #[inline(always)]
//     fn serialize_variant<'a>(ser: &'a mut Serializer<W, Self>, _variant_index: u32, variant_name: &'static str) -> Result<()>
//         where &'a mut Serializer<W, Self>: serde::ser::Serializer<Ok=(), Error=SerError>
//     {
//         ser.serialize_str(variant_name)
//     }
//     #[inline(always)]
//     fn serialize_struct<'a>(ser: &'a mut Serializer<W, Self>, len: usize) -> Result<Self::SerializeStruct>
//         where &'a mut Serializer<W, Self>: serde::ser::Serializer<Ok=(), Error=SerError,
//                 SerializeStruct=Self::SerializeStruct,
//                 SerializeStructVariant=Self::SerializeStruct>
//     {
//         write_map_len(&mut ser.output, len)?;
//         Ok(SerializeStruct::new(ser))
//     }    
// }

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
/// Serialize as a MessagePack message to a vector of bytes
///
/// Serialize data structures as arrays without field names and enum variants as indexes.
pub fn to_vec<T>(vec: &mut Vec<u8>, value: &T) -> Result<()>
    where T: Serialize,
{
    to_writer(vec, value)
}

// #[cfg(any(feature = "std", feature = "alloc"))]
// #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
// /// Serialize as a MessagePack message to a vector of bytes
// ///
// /// Serialize data structures as maps where resulting message will contain field and enum variant names.
// pub fn to_vec_named<T>(vec: &mut Vec<u8>, value: &T) -> Result<()>
//     where T: Serialize,
// {
//     to_writer_named(vec, value)
// }

/// Serialize as a MessagePack message to a SerWrite implementation
///
/// Serialize data structures as arrays without field names and enum variants as indexes.
pub fn to_writer<W, T>(writer: W, value: &T) -> Result<()>
    where W: SerWrite, T: Serialize
{
    let mut serializer = StructCompactSerializer::new(writer);
    value.serialize(&mut serializer)
}

// /// Serialize as a MessagePack message to a SerWrite implementation
// ///
// /// Serialize data structures as maps where resulting message will contain field and enum variant names.
// pub fn to_writer_named<W, T>(writer: W, value: &T) -> Result<()>
//     where W: SerWrite, T: Serialize
// {
//     let mut serializer = StructFieldNameSerializer::new(writer);
//     value.serialize(&mut serializer)
// }

pub trait StructSerializer<'a> {
    type SerializeStruct;

    fn serialize_variant(&mut self, variant_index: u32, variant_name: &'static str) -> Result<()>;
    fn serialize_struct_impl(&'a mut self, len: usize) -> Result<Self::SerializeStruct>;
}

impl<'a, W: SerWrite + 'a> StructSerializer<'a> for Serializer<W> {
    type SerializeStruct = SerializeStructArray<'a, Serializer<W>>;
    fn serialize_variant(&mut self, v: u32, _variant_name: &'static str) -> Result<()> {
        write_u32(&mut self.output, v)
    }

    fn serialize_struct_impl(&'a mut self, len: usize) -> Result<Self::SerializeStruct> {
        write_array_len(&mut self.output, len)?;
        Ok(SerializeStructArray { ser: self })
    }
}

// macro_rules! implement_serializer {
//     ($serializer:ty) => {};
// }
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

    type SerializeSeq = SerializeSeqMap<'a, Serializer<W>>;
    type SerializeTuple = SerializeTuple<'a, Serializer<W>>;
    type SerializeTupleStruct = SerializeTuple<'a, Serializer<W>>;
    type SerializeTupleVariant = SerializeTuple<'a, Serializer<W>>;
    type SerializeMap = SerializeSeqMap<'a, Serializer<W>>;
    type SerializeStruct = <Serializer<W> as StructSerializer<'a>>::SerializeStruct;
    type SerializeStructVariant = <Serializer<W> as StructSerializer<'a>>::SerializeStruct;

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
        write_u32(&mut self.output, v)
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
        variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.serialize_variant(variant_index, variant)
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
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.output.write_byte(FIXMAP|1)?;
        self.serialize_variant(variant_index, variant)?;
        value.serialize(&mut *self)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        let len = len.ok_or(SerError::SeqLength)?;
        write_array_len(&mut self.output, len)?;
        Ok(SerializeSeqMap { len, ser: self })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        write_array_len(&mut self.output, len)?;
        Ok(SerializeTuple { ser: self })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
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
    ) -> Result<Self::SerializeTupleVariant> {
        self.output.write_byte(FIXMAP|1)?;
        self.serialize_variant(variant_index, variant)?;
        self.serialize_tuple(len)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        let len = len.ok_or(SerError::MapLength)?;
        write_map_len(&mut self.output, len)?;
        Ok(SerializeSeqMap { len, ser: self })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct> {
        self.serialize_struct_impl(len)
    }

    // Struct variants are represented in JSON as `{ NAME: { K: V, ... } }`.
    // This is the externally tagged representation.
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.output.write_byte(FIXMAP|1)?;
        self.serialize_variant(variant_index, variant)?;
        self.serialize_struct_impl(len)
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

#[inline]
fn write_array_len<W: SerWrite>(output: &mut W, len: usize) -> Result<()> {
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
        return Err(SerError::BufferFull)
    }
    Ok(())
}

#[inline]
fn write_map_len<W: SerWrite>(output: &mut W, len: usize) -> Result<()> {
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
        return Err(SerError::BufferFull)
    }
    Ok(())
}

#[inline]
fn write_u32<W: SerWrite>(output: &mut W, v: u32) -> Result<()> {
    if v <= MAX_POSFIXINT as u32 {
        output.write_byte(v as u8)
    }
    else if let Ok(v) = u8::try_from(v) {
        output.write_byte(UINT_8)?;
        output.write_byte(v)
    }
    else if let Ok(v) = u16::try_from(v) {
        output.write_byte(UINT_16)?;
        output.write(&v.to_be_bytes())
    }
    else {
        output.write_byte(UINT_32)?;
        output.write(&v.to_be_bytes())
    }
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
impl<'a, S> ser::SerializeSeq for SerializeSeqMap<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = SerError>
{
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

impl<'a, S> ser::SerializeTuple for SerializeTuple<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = SerError>
{
    type Ok = ();
    type Error = SerError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where T: ?Sized + Serialize
    {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, S> ser::SerializeTupleStruct for SerializeTuple<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = SerError>
{
    type Ok = ();
    type Error = SerError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
        where T: ?Sized + Serialize
    {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

// Tuple variants are a little different. { NAME: [ ... ]}
impl<'a, S> ser::SerializeTupleVariant for SerializeTuple<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = SerError>
{
    type Ok = ();
    type Error = SerError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where T: ?Sized + Serialize
    {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, S> ser::SerializeMap for SerializeSeqMap<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = SerError>
{
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

impl<'a, S> ser::SerializeStruct for SerializeStructArray<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = SerError>
{
    type Ok = ();
    type Error = SerError;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<()>
        where T: ?Sized + Serialize
    {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, S> ser::SerializeStructVariant for SerializeStructArray<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = SerError>
{
    type Ok = ();
    type Error = SerError;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<()>
        where T: ?Sized + Serialize
    {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, S> ser::SerializeStruct for SerializeStructMap<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = SerError>
{
    type Ok = ();
    type Error = SerError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
        where T: ?Sized + Serialize
    {
        self.ser.serialize_str(key)?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, S> ser::SerializeStructVariant for SerializeStructMap<'a, S>
    where for<'b> &'b mut S: serde::Serializer<Ok = (), Error = SerError>
{
    type Ok = ();
    type Error = SerError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
        where T: ?Sized + Serialize
    {
        self.ser.serialize_str(key)?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_msgpack() {

//         #[derive(Serialize)]
//         struct Unit;
//         #[derive(Serialize)]
//         struct Test {
//             compact: bool,
//             schema: u32,
//             unit: Unit
//         }

//         let test = Test {
//             compact: true,
//             schema: 0,
//             unit: Unit
//         };
//         let expected = b"\x83\xA7compact\xC3\xA6schema\x00\xA4unit\xC0";
//         let mut vec = Vec::new();
//         to_vec_named(&mut vec, &test).unwrap();
//         assert_eq!(&vec, expected);
//         let expected = b"\x93\xA7\xC3\xA6\x00\xA4\xC0";
//         vec.clear();
//         to_vec(&mut vec, &test).unwrap();
//         assert_eq!(&vec, expected);
//     }
// }
