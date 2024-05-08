#![no_std]
#![no_main]
use panic_halt as _;
use cortex_m_rt::entry;

use serde::{Serialize, Deserialize};
use ser_write::{SliceWriter};

#[path = "../custom_bytes.rs"]
mod custom_bytes;
use custom_bytes::*;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Test<'a> {
    message: &'a str,
    number: f32,
    #[serde(with = "serde_bytes")]
    blob: &'a[u8],
}

#[entry]
fn main() -> ! {
    let mut container = [0u8; 256];
    let mut writer = SliceWriter::new(&mut container);
    let test = Test {
        message: "Hello world!",
        number: core::f32::consts::PI,
        blob: &[0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233],
    };
    to_writer(&mut writer, &test).unwrap();
    let s = core::str::from_utf8(writer.as_ref()).unwrap();
    assert_eq!(s, r#"{"message":"Hello world!","number":3.1415927,"blob":[0,1,1,2,3,5,8,13,21,34,55,89,144,233]}"#);

    let detest: Test = from_mut_slice_any_bytes(writer.as_mut()).unwrap();
    assert_eq!(detest, test);

    writer.clear();
    to_writer_hex_bytes(&mut writer, &test).unwrap();
    let s = core::str::from_utf8(writer.as_ref()).unwrap();
    assert_eq!(s, r#"{"message":"Hello world!","number":3.1415927,"blob":"hex,000101020305080D1522375990E9"}"#);

    let detest: Test = from_mut_slice_any_bytes(writer.as_mut()).unwrap();
    assert_eq!(detest, test);

    writer.clear();
    to_writer_base64_bytes(&mut writer, &test).unwrap();
    let s = core::str::from_utf8(writer.as_ref()).unwrap();
    assert_eq!(s, r#"{"message":"Hello world!","number":3.1415927,"blob":"base64,AAEBAgMFCA0VIjdZkOk"}"#);

    let detest: Test = from_mut_slice_any_bytes(writer.as_mut()).unwrap();
    assert_eq!(detest, test);

    loop {}
}
