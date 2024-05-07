use core::marker::PhantomData;
use core::fmt;
use core::mem::MaybeUninit;

#[cfg(feature = "std")]
use std::{vec::Vec, string::String};

#[cfg(all(feature = "alloc",not(feature = "std")))]
use alloc::{vec::Vec, string::String};

use serde::{ser, Serialize};
use ser_write::{SerResult as Result};

pub use ser_write::{SerWrite, SerError, SerResult};

/// JSON serializer serializing bytes to an array of numbers
pub type SerializerByteArray<W> = Serializer<W, ArrayByteFormatter>;
/// JSON serializer serializing bytes to a hexadecimal string
pub type SerializerByteHexStr<W> = Serializer<W, HexStrByteFormatter>;
/// JSON serializer passing bytes through
pub type SerializerBytePass<W> = Serializer<W, PassThroughByteFormatter>;

pub struct Serializer<W, B> {
    first: bool,
    output: W,
    format: PhantomData<B>
}

/// Provides `serialize_bytes` implementation
pub trait BytesFormat: Sized {
    fn serialize_bytes<'a, W: SerWrite>(ser: &'a mut Serializer<W, Self>, v: &[u8]) -> Result<()>
        where &'a mut Serializer<W, Self>: serde::ser::Serializer<Ok=(), Error=SerError>;
}

/// Implements [`BytesFormat::serialize_bytes`] serializing to an array of numbers
pub struct ArrayByteFormatter;
/// Implements [`BytesFormat::serialize_bytes`] serializing to a hexadecimal string
pub struct HexStrByteFormatter;
/// Implements [`BytesFormat::serialize_bytes`] passing bytes through
pub struct PassThroughByteFormatter;

impl BytesFormat for ArrayByteFormatter {
    fn serialize_bytes<'a, W: SerWrite>(ser: &'a mut Serializer<W, Self>, v: &[u8]) -> Result<()>
        where &'a mut Serializer<W, Self>: serde::ser::Serializer<Ok=(), Error=SerError>
    {
        use serde::ser::{Serializer, SerializeSeq};
        let mut seq = ser.serialize_seq(Some(v.len()))?;
        for byte in v {
            seq.serialize_element(byte)?;
        }
        seq.end()
    }
}

impl BytesFormat for HexStrByteFormatter {
    fn serialize_bytes<'a, W: SerWrite>(ser: &'a mut Serializer<W, Self>, v: &[u8]) -> Result<()>
        where &'a mut Serializer<W, Self>: serde::ser::Serializer<Ok=(), Error=SerError>
    {
        ser.output.write_byte(b'"')?;
        for &byte in v.iter() {
            ser.output.write(&hex(byte))?;
        }
        ser.output.write_byte(b'"')
    }
}

impl BytesFormat for PassThroughByteFormatter {
    fn serialize_bytes<'a, W: SerWrite>(ser: &'a mut Serializer<W, Self>, v: &[u8]) -> Result<()>
        where &'a mut Serializer<W, Self>: serde::ser::Serializer<Ok=(), Error=SerError>
    {
        ser.output.write(v)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub fn to_string<T>(value: &T) -> Result<String>
    where T: Serialize + ?Sized
{
    let mut vec = Vec::new();
    to_writer(&mut vec, value)?;
    Ok(unsafe { String::from_utf8_unchecked(vec) })
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub fn to_string_hex_bytes<T>(value: &T) -> Result<String>
    where T: Serialize + ?Sized
{
    let mut vec = Vec::new();
    to_writer_hex_bytes(&mut vec, value)?;
    Ok(unsafe { String::from_utf8_unchecked(vec) })
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub fn to_string_pass_bytes<T>(value: &T) -> Result<String>
    where T: Serialize + ?Sized
{
    let mut vec = Vec::new();
    to_writer_pass_bytes(&mut vec, value)?;
    String::from_utf8(vec).map_err(|_| SerError::OtherError)
}

/// Serialize JSON to a SerWrite implementation
///
/// Serialize bytes as arrays of numbers.
pub fn to_writer<W, T>(writer: W, value: &T) -> Result<()>
    where W: SerWrite, T: Serialize + ?Sized
{
    let mut serializer = SerializerByteArray::new(writer);
    value.serialize(&mut serializer)
}

/// Serialize JSON to a SerWrite implementation
///
/// Serialize bytes as hex strings.
pub fn to_writer_hex_bytes<W, T>(writer: W, value: &T) -> Result<()>
    where W: SerWrite, T: Serialize + ?Sized
{
    let mut serializer = SerializerByteHexStr::new(writer);
    value.serialize(&mut serializer)
}

/// Serialize JSON to a SerWrite implementation, passing through byte arrays
///
/// Serialize bytes passing them through.
/// The notion here is that byte arrays can hold already serialized JSON fragments.
pub fn to_writer_pass_bytes<W, T>(writer: W, value: &T) -> Result<()>
    where W: SerWrite, T: Serialize + ?Sized
{
    let mut serializer = SerializerBytePass::new(writer);
    value.serialize(&mut serializer)
}

impl<W, B> Serializer<W, B> {
    #[inline(always)]
    pub fn new(output: W) -> Self {
        Serializer { first: false, output, format: PhantomData }
    }

    #[inline(always)]
    pub fn into_inner(self) -> W {
        self.output
    }
}

#[inline(always)]
fn hex_4bit(c: u8) -> u8 {
    if c <= 9 {
        0x30 + c
    } else {
        0x41 + (c - 10)
    }
}

/// Upper-case hex for value in 0..256, encoded as ASCII bytes
#[inline(always)]
fn hex(c: u8) -> [u8;2] {
    [hex_4bit(c >> 4), hex_4bit(c & 0x0F)]
}

macro_rules! serialize_unsigned {
    ($self:ident, $N:expr, $v:expr) => {{
        let mut buf: [MaybeUninit<u8>; $N] = unsafe {
            MaybeUninit::<[MaybeUninit<u8>; $N]>::uninit().assume_init()
        };

        let mut v = $v;
        let mut i = $N - 1;
        loop {
            buf[i].write((v % 10) as u8 + b'0');
            v /= 10;

            if v == 0 {
                break;
            } else {
                i -= 1;
            }
        }

        // Note(feature): maybe_uninit_slice
        let buf = unsafe { &*(&buf[i..] as *const _ as *const [u8]) };

        $self.output.write(buf)
    }};
}

macro_rules! serialize_signed {
    ($self:ident, $N:expr, $v:expr, $ixx:ident, $uxx:ident) => {{
        let v = $v;
        let (signed, mut v) = if v == $ixx::min_value() {
            (true, $ixx::max_value() as $uxx + 1)
        } else if v < 0 {
            (true, -v as $uxx)
        } else {
            (false, v as $uxx)
        };

        let mut buf: [MaybeUninit<u8>; $N] = unsafe {
            MaybeUninit::<[MaybeUninit<u8>; $N]>::uninit().assume_init()
        };
        let mut i = $N - 1;
        loop {
            buf[i].write((v % 10) as u8 + b'0');
            v /= 10;

            i -= 1;

            if v == 0 {
                break;
            }
        }

        if signed {
            buf[i].write(b'-');
        } else {
            i += 1;
        }

        // Note(feature): maybe_uninit_slice
        let buf = unsafe { &*(&buf[i..] as *const _ as *const [u8]) };

        $self.output.write(buf)
    }};
}

macro_rules! serialize_ryu {
    ($self:ident, $v:expr) => {{
        let mut buffer = ryu_js::Buffer::new();
        let printed = buffer.format_finite($v);
        $self.output.write_str(printed)
    }};
}

impl<'a, W: SerWrite, B: BytesFormat> ser::Serializer for &'a mut Serializer<W, B> {
    type Ok = ();
    type Error = SerError;

    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.output.write(if v { b"true" } else { b"false" })
    }
    #[inline(always)]
    fn serialize_i8(self, v: i8) -> Result<()> {
        self.serialize_i32(i32::from(v))
    }
    #[inline(always)]
    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_i32(i32::from(v))
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        // "-2147483648"
        serialize_signed!(self, 11, v, i32, u32)
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        // "-9223372036854775808"
        serialize_signed!(self, 20, v, i64, u64)
    }
    #[inline(always)]
    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_u32(u32::from(v))
    }
    #[inline(always)]
    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_u32(u32::from(v))
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok> {
        // "4294967295"
        serialize_unsigned!(self, 10, v)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok> {
        // "18446744073709551615"
        serialize_unsigned!(self, 20, v)
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        if v.is_finite() {
            serialize_ryu!(self, v)
        } else {
            self.serialize_none()
        }
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        if v.is_finite() {
            serialize_ryu!(self, v)
        } else {
            self.serialize_none()
        }
    }

    fn serialize_char(self, v: char) -> Result<()> {
        let mut encoding_tmp = [0u8; 4];
        let encoded = v.encode_utf8(&mut encoding_tmp);
        self.serialize_str(encoded)
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        self.output.write_byte(b'"')?;
        format_escaped_str_contents(&mut self.output, v)?;
        self.output.write_byte(b'"')
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        B::serialize_bytes(self, v)
    }

    fn serialize_none(self) -> Result<()> {
        self.output.write(b"null")
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
        self.output.write_byte(b'{')?;
        self.serialize_str(variant)?;
        self.output.write_byte(b':')?;
        value.serialize(&mut *self)?;
        self.output.write_byte(b'}')
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        self.output.write_byte(b'[')?;
        self.first = true;
        Ok(self)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(None)
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_seq(None)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.output.write_byte(b'{')?;
        self.serialize_str(variant)?;
        self.output.write(b":[")?;
        self.first = true;
        Ok(self)
    }

    // Maps are represented in JSON as `{ K: V, K: V, ... }`.
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        self.output.write_byte(b'{')?;
        self.first = true;
        Ok(self)
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct> {
        self.serialize_map(None)
    }

    // Struct variants are represented in JSON as `{ NAME: { K: V, ... } }`.
    // This is the externally tagged representation.
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.output.write_byte(b'{')?;
        self.serialize_str(variant)?;
        self.output.write(b":{")?;
        self.first = true;
        Ok(self)
    }

    fn collect_str<T: ?Sized>(self, value: &T) -> Result<Self::Ok>
        where T: fmt::Display
    {
        self.output.write_byte(b'"')?;
        let mut col = StringCollector::new(&mut self.output);
        fmt::write(&mut col, format_args!("{}", value)).map_err(|_| SerError::BufferFull)?;
        self.output.write_byte(b'"')
    }
}

struct StringCollector<'a, W> {
    output: &'a mut W,
}

impl<'a, W> StringCollector<'a, W> {
    #[inline(always)]
    pub fn new(output: &'a mut W) -> Self {
        Self { output }
    }
}

impl<'a, W: SerWrite> fmt::Write for StringCollector<'a, W> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        format_escaped_str_contents(self.output, s).map_err(|_| fmt::Error)
    }
}

// This impl is SerializeSeq so these methods are called after `serialize_seq`
// is called on the Serializer.
impl<'a, W: SerWrite, B: BytesFormat> ser::SerializeSeq for &'a mut Serializer<W, B> {
    type Ok = ();
    type Error = SerError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
        where T: ?Sized + Serialize
    {
        if self.first {
            self.first = false;
        }
        else {
            self.output.write_byte(b',')?;
        }
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.first = false;
        self.output.write_byte(b']')
    }
}

impl<'a, W: SerWrite, B: BytesFormat> ser::SerializeTuple for &'a mut Serializer<W, B> {
    type Ok = ();
    type Error = SerError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where T: ?Sized + Serialize
    {
        if self.first {
            self.first = false;
        }
        else {
            self.output.write_byte(b',')?;
        }
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.first = false;
        self.output.write_byte(b']')
    }
}

impl<'a, W: SerWrite, B: BytesFormat> ser::SerializeTupleStruct for &'a mut Serializer<W, B> {
    type Ok = ();
    type Error = SerError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
        where T: ?Sized + Serialize
    {
        if self.first {
            self.first = false;
        }
        else {
            self.output.write_byte(b',')?;
        }
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.first = false;
        self.output.write_byte(b']')
    }
}

// Tuple variants are a little different. { NAME: [ ... ]}
impl<'a, W: SerWrite, B: BytesFormat> ser::SerializeTupleVariant for &'a mut Serializer<W, B> {
    type Ok = ();
    type Error = SerError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where T: ?Sized + Serialize
    {
        if self.first {
            self.first = false;
        }
        else {
            self.output.write_byte(b',')?;
        }
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.first = false;
        self.output.write(b"]}")
    }
}

impl<'a, W: SerWrite, B: BytesFormat> ser::SerializeMap for &'a mut Serializer<W, B> {
    type Ok = ();
    type Error = SerError;

    /// The Serde data model allows map keys to be any serializable type.
    /// JSON only allows string keys so the implementation below will produce invalid
    /// JSON if the key serializes as something other than a string.
    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
        where T: ?Sized + Serialize
    {
        if self.first {
            self.first = false;
        }
        else {
            self.output.write_byte(b',')?;
        }
        key.serialize(&mut **self)
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where T: ?Sized + Serialize
    {
        self.output.write(b":")?;
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.first = false;
        self.output.write_byte(b'}')
    }
}

impl<'a, W: SerWrite, B: BytesFormat> ser::SerializeStruct for &'a mut Serializer<W, B> {
    type Ok = ();
    type Error = SerError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
        where T: ?Sized + Serialize
    {
        if self.first {
            self.first = false;
        }
        else {
            self.output.write_byte(b',')?;
        }
        key.serialize(&mut **self)?;
        self.output.write(b":")?;
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.first = false;
        self.output.write_byte(b'}')
    }
}

impl<'a, W: SerWrite, B: BytesFormat> ser::SerializeStructVariant for &'a mut Serializer<W, B> {
    type Ok = ();
    type Error = SerError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
        where T: ?Sized + Serialize
    {
        if self.first {
            self.first = false;
        }
        else {
            self.output.write_byte(b',')?;
        }
        key.serialize(&mut **self)?;
        self.output.write(b":")?;
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.first = false;
        self.output.write(b"}}")
    }
}

fn format_escaped_str_contents<W>(
    writer: &mut W,
    value: &str,
) -> Result<()>
    where W: ?Sized + SerWrite
{
    let bytes = value.as_bytes();

    let mut start = 0;

    for (i, &byte) in bytes.iter().enumerate() {
        let escape = match byte {
            0x00..=0x1F => ESCAPE[byte as usize],
            QU|BS => byte,
            _ => continue
        };

        if start < i {
            writer.write_str(&value[start..i])?;
        }

        if escape == UU {
            writer.write(b"\\u00")?;
            writer.write(&hex(byte))?;
        }
        else {
            writer.write(&[b'\\', escape])?;
        }

        start = i + 1;
    }

    if start == bytes.len() {
        return Ok(());
    }

    writer.write_str(&value[start..])
}

const BB: u8 = b'b'; // \x08
const TT: u8 = b't'; // \x09
const NN: u8 = b'n'; // \x0A
const FF: u8 = b'f'; // \x0C
const RR: u8 = b'r'; // \x0D
const QU: u8 = b'"'; // \x22
const BS: u8 = b'\\'; // \x5C
const UU: u8 = b'u'; // \x00...\x1F except the ones above

// Lookup table of escape sequences. A value of b'x' at index i means that byte
// i is escaped as "\x" in JSON.
static ESCAPE: [u8; 32] = [
    //   1   2   3   4   5   6   7   8   9   A   B   C   D   E   F
    UU, UU, UU, UU, UU, UU, UU, UU, BB, TT, NN, UU, FF, RR, UU, UU, // 0
    UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, // 1
];

#[cfg(test)]
mod tests {
    use std::{vec, vec::Vec};
    use super::*;

    #[test]
    fn test_json_tuple() {
        #[derive(Serialize)]
        struct Test {
            int: u32,
            seq: Vec<&'static str>,
        }

        let test = Test {
            int: 1,
            seq: vec!["a", "b"],
        };
        let expected = r#"[100000,"bam!",0.4,{"int":1,"seq":["a","b"]},null]"#;
        let tup = (100000u64,"bam!",0.4f64,test,0.0f64/0.0);
        assert_eq!(to_string(&tup).unwrap(), expected);
    }

    #[test]
    fn test_json_struct() {
        #[derive(Serialize)]
        struct Test {
            int: u32,
            seq: Vec<&'static str>,
        }

        let test = Test {
            int: 1,
            seq: vec!["a", "b"],
        };
        let expected = r#"{"int":1,"seq":["a","b"]}"#;
        assert_eq!(to_string(&test).unwrap(), expected);
    }

    #[test]
    fn test_json_enum() {
        #[derive(Serialize)]
        enum E {
            Unit,
            Newtype(u32),
            Tuple(u32, f32),
            Struct { a: u32 },
        }

        let u = E::Unit;
        let expected = r#""Unit""#;
        assert_eq!(to_string(&u).unwrap(), expected);

        let n = E::Newtype(1);
        let expected = r#"{"Newtype":1}"#;
        assert_eq!(to_string(&n).unwrap(), expected);

        let t = E::Tuple(1, std::f32::consts::PI);
        let expected = r#"{"Tuple":[1,3.1415927]}"#;
        assert_eq!(to_string(&t).unwrap(), expected);

        let s = E::Struct { a: 1 };
        let expected = r#"{"Struct":{"a":1}}"#;
        assert_eq!(to_string(&s).unwrap(), expected);
    }

    #[test]
    fn test_json_string() {
        let s = "\"\x00\x08\x09\n\x0C\rłączka\x1f\\\x7f\"";
        let expected = "\"\\\"\\u0000\\b\\t\\n\\f\\rłączka\\u001F\\\\\x7f\\\"\"";
        assert_eq!(to_string(&s).unwrap(), expected);
    }

    #[test]
    fn test_json_bytes() {
        #[derive(Serialize)]
        struct Test {
            #[serde(with = "serde_bytes")]
            key: Vec<u8>,
        }
        let mut vec = Vec::new();
        let expected = r#"[{"key":{"Struct":{"a":1}}}]"#;
        let value = [Test { key: r#"{"Struct":{"a":1}}"#.as_bytes().into() }];
        to_writer_pass_bytes(&mut vec, &value).unwrap();
        let s = String::from_utf8(vec).unwrap();
        assert_eq!(s, expected);
        let expected = r#"[{"key":"7B22537472756374223A7B2261223A317D7D"}]"#;
        assert_eq!(&to_string_hex_bytes(&value).unwrap(), expected);
        let expected = r#"[{"key":[123,34,83,116,114,117,99,116,34,58,123,34,97,34,58,49,125,125]}]"#;
        assert_eq!(&to_string(&value).unwrap(), expected);
    }

    #[test]
    fn test_ser_array() {
        let mut buf = Vec::new();
        to_writer(&mut buf, &[0, 1, 2]).unwrap();
        assert_eq!(&buf[..], b"[0,1,2]");
    }

    #[test]
    fn test_ser_bool() {
        let mut buf = Vec::new();
        to_writer(&mut buf, &true).unwrap();
        assert_eq!(&buf[..], b"true");
        buf.clear();
        to_writer(&mut buf, &false).unwrap();
        assert_eq!(&buf[..], b"false");
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

        assert_eq!(
            &*to_string(&Type::Boolean).unwrap(),
            r#""boolean""#
        );

        assert_eq!(
            &*to_string(&Type::Number).unwrap(),
            r#""number""#
        );
    }

    #[test]
    fn test_ser_str() {
        assert_eq!(&*to_string("hello").unwrap(), r#""hello""#);
        assert_eq!(&*to_string("").unwrap(), r#""""#);

        // Characters unescaped if possible
        assert_eq!(&*to_string("ä").unwrap(), r#""ä""#);
        assert_eq!(&*to_string("৬").unwrap(), r#""৬""#);
        assert_eq!(&*to_string("\u{A0}").unwrap(), "\"\u{A0}\""); // non-breaking space
        assert_eq!(&*to_string("ℝ").unwrap(), r#""ℝ""#); // 3 byte character
        assert_eq!(&*to_string("💣").unwrap(), r#""💣""#); // 4 byte character

        // " and \ must be escaped
        assert_eq!(
            &*to_string("foo\"bar").unwrap(),
            r#""foo\"bar""#
        );
        assert_eq!(
            &*to_string("foo\\bar").unwrap(),
            r#""foo\\bar""#
        );

        // \b, \t, \n, \f, \r must be escaped in their two-character escaping
        assert_eq!(
            &*to_string(" \u{0008} ").unwrap(),
            r#"" \b ""#
        );
        assert_eq!(
            &*to_string(" \u{0009} ").unwrap(),
            r#"" \t ""#
        );
        assert_eq!(
            &*to_string(" \u{000A} ").unwrap(),
            r#"" \n ""#
        );
        assert_eq!(
            &*to_string(" \u{000C} ").unwrap(),
            r#"" \f ""#
        );
        assert_eq!(
            &*to_string(" \u{000D} ").unwrap(),
            r#"" \r ""#
        );

        // U+0000 through U+001F is escaped using six-character \u00xx uppercase hexadecimal escape sequences
        assert_eq!(
            &*to_string(" \u{0000} ").unwrap(),
            r#"" \u0000 ""#
        );
        assert_eq!(
            &*to_string(" \u{0001} ").unwrap(),
            r#"" \u0001 ""#
        );
        assert_eq!(
            &*to_string(" \u{0007} ").unwrap(),
            r#"" \u0007 ""#
        );
        assert_eq!(
            &*to_string(" \u{000e} ").unwrap(),
            r#"" \u000E ""#
        );
        assert_eq!(
            &*to_string(" \u{001D} ").unwrap(),
            r#"" \u001D ""#
        );
        assert_eq!(
            &*to_string(" \u{001f} ").unwrap(),
            r#"" \u001F ""#
        );
    }

    #[test]
    fn test_ser_struct_bool() {
        #[derive(Serialize)]
        struct Led {
            led: bool,
        }

        assert_eq!(
            &*to_string(&Led { led: true }).unwrap(),
            r#"{"led":true}"#
        );
    }

    #[test]
    fn test_ser_struct_i8() {
        #[derive(Serialize)]
        struct Temperature {
            temperature: i8,
        }

        assert_eq!(
            &*to_string(&Temperature { temperature: 127 }).unwrap(),
            r#"{"temperature":127}"#
        );

        assert_eq!(
            &*to_string(&Temperature { temperature: 20 }).unwrap(),
            r#"{"temperature":20}"#
        );

        assert_eq!(
            &*to_string(&Temperature { temperature: -17 }).unwrap(),
            r#"{"temperature":-17}"#
        );

        assert_eq!(
            &*to_string(&Temperature { temperature: -128 }).unwrap(),
            r#"{"temperature":-128}"#
        );
    }

    #[test]
    fn test_ser_struct_f32() {
        #[derive(Serialize)]
        struct Temperature {
            temperature: f32,
        }

        assert_eq!(
            &*to_string(&Temperature { temperature: -20.0 }).unwrap(),
            r#"{"temperature":-20}"#
        );

        assert_eq!(
            &*to_string(&Temperature {
                temperature: -20345.
            })
            .unwrap(),
            r#"{"temperature":-20345}"#
        );

        assert_eq!(
            &*to_string(&Temperature {
                temperature: -2.3456789012345e-23
            })
            .unwrap(),
            r#"{"temperature":-2.3456788e-23}"#
        );

        assert_eq!(
            &*to_string(&Temperature {
                temperature: f32::NAN
            })
            .unwrap(),
            r#"{"temperature":null}"#
        );

        assert_eq!(
            &*to_string(&Temperature {
                temperature: f32::NEG_INFINITY
            })
            .unwrap(),
            r#"{"temperature":null}"#
        );
    }

    #[test]
    fn test_ser_struct_option() {
        #[derive(Serialize)]
        struct Property<'a> {
            description: Option<&'a str>,
        }

        assert_eq!(
            to_string(&Property {
                description: Some("An ambient temperature sensor"),
            })
            .unwrap(),
            r#"{"description":"An ambient temperature sensor"}"#
        );

        // XXX Ideally this should produce "{}"
        assert_eq!(
            to_string(&Property { description: None }).unwrap(),
            r#"{"description":null}"#
        );
    }

    #[test]
    fn test_ser_struct_u8() {
        #[derive(Serialize)]
        struct Temperature {
            temperature: u8,
        }

        assert_eq!(
            &*to_string(&Temperature { temperature: 20 }).unwrap(),
            r#"{"temperature":20}"#
        );
    }

    #[test]
    fn test_ser_struct_() {
        #[derive(Serialize)]
        struct Empty {}

        assert_eq!(&*to_string(&Empty {}).unwrap(), r#"{}"#);

        #[derive(Serialize)]
        struct Tuple {
            a: bool,
            b: bool,
        }

        assert_eq!(
            &*to_string(&Tuple { a: true, b: false }).unwrap(),
            r#"{"a":true,"b":false}"#
        );
    }

    #[test]
    fn test_ser_unit() {
        let a = ();
        assert_eq!(&*to_string(&a).unwrap(), r#"null"#);
    }

    #[test]
    fn test_ser_newtype_struct() {
        #[derive(Serialize)]
        struct A(pub u32);
        let a = A(54);
        assert_eq!(&*to_string(&a).unwrap(), r#"54"#);
    }

    #[test]
    fn test_ser_newtype_variant() {
        #[derive(Serialize)]
        enum A {
            A(u32),
        }
        let a = A::A(54);

        assert_eq!(&*to_string(&a).unwrap(), r#"{"A":54}"#);
    }

    #[test]
    fn test_ser_struct_variant() {
        #[derive(Serialize)]
        enum A {
            A { x: u32, y: u16 },
        }
        let a = A::A { x: 54, y: 720 };

        assert_eq!(
            &*to_string(&a).unwrap(),
            r#"{"A":{"x":54,"y":720}}"#
        );
    }

    #[test]
    fn test_ser_tuple_struct() {
        #[derive(Serialize)]
        struct A<'a>(u32, Option<&'a str>, u16, bool);

        let a = A(42, Some("A string"), 720, false);

        assert_eq!(
            &*to_string(&a).unwrap(),
            r#"[42,"A string",720,false]"#
        );
    }

    #[test]
    fn test_ser_tuple_struct_roundtrip() {
        use serde::Deserialize;

        #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
        struct A<'a>(u32, Option<&'a str>, u16, bool);

        let a1 = A(42, Some("A string"), 720, false);
        let mut serialized = to_string(&a1).unwrap().into_bytes();
        let a2: A<'_> = crate::from_mut_slice(&mut serialized).unwrap();
        assert_eq!(a1, a2);
    }

    #[test]
    fn test_ser_serialize_bytes() {
        use core::fmt::Write;

        pub struct SimpleDecimal(f32);

        impl serde::Serialize for SimpleDecimal {
            fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
                where S: serde::Serializer
            {
                let mut aux = String::new();
                write!(aux, "{:.2}", self.0).unwrap();
                serializer.serialize_bytes(&aux.as_bytes())
            }
        }

        let sd1 = SimpleDecimal(1.55555);
        assert_eq!(&*to_string_pass_bytes(&sd1).unwrap(), r#"1.56"#);

        let sd2 = SimpleDecimal(0.000);
        assert_eq!(&*to_string_pass_bytes(&sd2).unwrap(), r#"0.00"#);

        let sd3 = SimpleDecimal(22222.777777);
        assert_eq!(&*to_string_pass_bytes(&sd3).unwrap(), r#"22222.78"#);
    }
}
