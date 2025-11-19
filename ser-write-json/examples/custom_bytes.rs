//! An example demonstrating how to implement custom bytes decoder/encoder traits
#![cfg_attr(not(feature = "std"), allow(dead_code))]
use core::fmt;
use serde::{Serialize, Deserialize, de};
use ser_write_json::{
    base64,
    ser_write::SerWrite,
    ser::{Error, Serializer, ByteEncoder},
    to_writer_with_encoder,
    de::{StringByteDecoder, Deserializer, Result as DeResult},
    from_mut_slice_with_decoder,
};

pub use ser_write_json::to_writer;

macro_rules! prefix_base64 { () => {
    "base64,"
};}
macro_rules! prefix_hex { () => {
    "hex,"
};}

/* Serializer */

pub fn to_writer_hex_bytes<W, T>(writer: W, value: &T) -> Result<(), Error<W::Error>>
    where W: SerWrite,
          <W as SerWrite>::Error: fmt::Display + fmt::Debug,
          T: Serialize + ?Sized
{
    to_writer_with_encoder::<PrefixHexByteEncoder, _, _>(writer, value)
}

/// Serialize bytes as HEX-encoded strings with a prefix
pub struct PrefixHexByteEncoder;

impl ByteEncoder for PrefixHexByteEncoder {
    fn serialize_bytes<'a, W: SerWrite>(ser: &'a mut Serializer<W, Self>, v: &[u8]) -> Result<(), Error<W::Error>>
        where &'a mut Serializer<W, Self>: serde::ser::Serializer<Ok=(), Error=Error<W::Error>>
    {
        ser.writer().write_str(concat!('"', prefix_hex!()))?;
        ser.serialize_bytes_as_hex_str(v)?;
        Ok(ser.writer().write_byte(b'"')?)

    }
}

pub fn to_writer_base64_bytes<W, T>(writer: W, value: &T) -> Result<(), Error<W::Error>>
    where W: SerWrite,
          <W as SerWrite>::Error: fmt::Display + fmt::Debug,
          T: Serialize + ?Sized
{
    to_writer_with_encoder::<PrefixBase64ByteEncoder, _, _>(writer, value)
}

/// Serialize bytes as Base64 strings with a prefix
pub struct PrefixBase64ByteEncoder;

impl ByteEncoder for PrefixBase64ByteEncoder {
    fn serialize_bytes<'a, W: SerWrite>(ser: &'a mut Serializer<W, Self>, v: &[u8]) -> Result<(), Error<W::Error>>
        where &'a mut Serializer<W, Self>: serde::ser::Serializer<Ok=(), Error=Error<W::Error>>
    {
        ser.writer().write_str(concat!('"', prefix_base64!()))?;
        base64::encode(ser.writer(), v)?;
        Ok(ser.writer().write_byte(b'"')?)

    }
}

/* Deserializer */

pub fn from_mut_slice_any_bytes<'a, T>(v: &'a mut [u8]) -> DeResult<T>
    where T: de::Deserialize<'a>
{
    from_mut_slice_with_decoder::<StringByteAnyDecoder, _>(v)
}

/// Deserialize bytes from strings depending on the prefix found in the string
pub struct StringByteAnyDecoder;

impl<'de> StringByteDecoder<'de> for StringByteAnyDecoder {
    fn decode_string_to_bytes(de: &mut Deserializer<'de, Self>) -> DeResult<&'de[u8]> {
        const HEX: &[u8] = prefix_hex!().as_bytes();
        const B64: &[u8] = prefix_base64!().as_bytes();
        let input = de.input_mut()?;
        if input.starts_with(B64) {
            de.eat_some(B64.len());
            de.parse_base64_bytes_content()
        }
        else if input.starts_with(HEX) {
            de.eat_some(HEX.len());
            de.parse_hex_bytes_content()
        }
        else {
            de.parse_str_bytes_content()
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Test<'a> {
    message: &'a str,
    number: f32,
    #[serde(with = "serde_bytes")]
    blob: &'a[u8],
}

#[cfg(not(feature = "std"))]
#[allow(dead_code)]
fn main() {}

#[cfg(feature = "std")]
fn main() {
    let test = Test {
        message: "Hello world!",
        number: core::f32::consts::PI,
        blob: &[0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233],
    };

    println!("Bytes as an array");
    let mut vec = Vec::new();
    to_writer(&mut vec, &test).unwrap();
    let s = String::from_utf8(vec).unwrap();
    println!("{}", s);
    assert_eq!(s, r#"{"message":"Hello world!","number":3.1415927,"blob":[0,1,1,2,3,5,8,13,21,34,55,89,144,233]}"#);

    let mut vec = s.into_bytes();
    let detest: Test = from_mut_slice_any_bytes(vec.as_mut_slice()).unwrap();
    println!("{:?}", detest);
    assert_eq!(detest, test);

    println!("Bytes as a HEX string");
    vec.clear();
    to_writer_hex_bytes(&mut vec, &test).unwrap();
    let s = String::from_utf8(vec).unwrap();
    println!("{}", s);
    assert_eq!(s, r#"{"message":"Hello world!","number":3.1415927,"blob":"hex,000101020305080D1522375990E9"}"#);

    let mut vec = s.into_bytes();
    let detest: Test = from_mut_slice_any_bytes(vec.as_mut_slice()).unwrap();
    println!("{:?}", detest);
    assert_eq!(detest, test);

    println!("Bytes as a BASE-64 string");
    vec.clear();
    to_writer_base64_bytes(&mut vec, &test).unwrap();
    let s = String::from_utf8(vec).unwrap();
    println!("{}", s);
    assert_eq!(s, r#"{"message":"Hello world!","number":3.1415927,"blob":"base64,AAEBAgMFCA0VIjdZkOk"}"#);

    let mut vec = s.into_bytes();
    let detest: Test = from_mut_slice_any_bytes(vec.as_mut_slice()).unwrap();
    println!("{:?}", detest);
    assert_eq!(detest, test);
}
