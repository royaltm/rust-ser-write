#![no_std]
#![no_main]
use panic_halt as _;
use cortex_m_rt::entry;

use serde::{Serialize, Deserialize};
use ser_write_msgpack::*;
use ser_write::{SliceWriter};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Test<'a> {
    message: &'a str,
    number: i32,
    #[serde(with = "serde_bytes")]
    blob: &'a[u8],
}

#[entry]
fn main() -> ! {
    let mut container = [0u8; 256];
    let mut writer = SliceWriter::new(&mut container);
    let test = Test {
        message: "Hello world!",
        number: -1,
        blob: &[0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233],
    };
    to_writer(&mut writer, &test).unwrap();
    let s = writer.as_ref();
    assert_eq!(s, b"\x83\x00\xACHello world!\x01\xff\x02\xC4\x0e\x00\x01\x01\x02\x03\x05\x08\x0d\x15\x22\x37\x59\x90\xe9");
    let detest: Test = from_slice(writer.as_mut()).unwrap().0;
    assert_eq!(detest, test);

    writer.clear();
    to_writer_compact(&mut writer, &test).unwrap();
    let s = writer.as_ref();
    assert_eq!(s, b"\x93\xACHello world!\xff\xC4\x0e\x00\x01\x01\x02\x03\x05\x08\x0d\x15\x22\x37\x59\x90\xe9");

    let detest: Test = from_slice(writer.as_mut()).unwrap().0;
    assert_eq!(detest, test);

    writer.clear();
    to_writer_named(&mut writer, &test).unwrap();
    let s = writer.as_ref();
    assert_eq!(s, b"\x83\xA7message\xACHello world!\xA6number\xff\xA4blob\xC4\x0e\x00\x01\x01\x02\x03\x05\x08\x0d\x15\x22\x37\x59\x90\xe9");

    let detest: Test = from_slice(writer.as_mut()).unwrap().0;
    assert_eq!(detest, test);

    loop {}
}
