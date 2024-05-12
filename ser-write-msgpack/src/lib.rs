//! A MessagePack serde serializer for [`ser-write`](`ser_write`) and a deserializer for convenience.
/*!

[`Serializer`] types:

| Serde type ->     | MessagePack type
|-------------------|--------------------
| `()`              | `nil`
| `Unit` struct     | `nil`
| `bool`            | `bool`
| `NewType(T)`      | `T` -> `MessagePack`
| `None`            | `nil`
| `Some(T)`         | `T` -> `MessagePack`
| `u8`-`u64`        | `uint` (smallest representation)
| `i8`-`i64`        | `int`, `uint` (sm. repr.)
| `f23`             | `float-32`
| `f64`             | `float-64`
| `str`             | `str`
| `bytes`           | `bin`
| `array`, `tuple`  | `array` (sm. repr.)
| `seq`-like        | `array` (sm. repr.)
| `map`-like        | `map` (sm. repr.)
| `struct`          | `map` or `array` (depending on implementation)
| `unit variant`    | `str` or `uint` (depending on implementation)
| `newtype variant` | `fixmap:1` `variant`, `T` (`variant`: `str` or `uint` impl. dep.)
| `tuple variant`   | `fixmap:1` `variant`, `array` (impl. dep.)
| `struct variant`  | `fixmap:1` `variant`, `struct` (impl. dep.)

Currently neither [`Serializer`] nor [`Deserializer`] supports MessagePack extension types.
The `ext` and `fixext` types are properly recognized and skipped over when a struct field is ignored.

[`Deserializer`] supports self-describing formats (`deserialize_any`).

[`Deserializer`] deserializes structs from both MessagePack maps or arrays using
both `uint` or `str` as field identifiers.

[`Deserializer`] types:

| MessagePack type -> | Serde type (depending on context)
|---------------------|----------------------------------------
| `nil`               | `unit`,`none`,`NaN`
| `bool`              | `bool`
| `fixint`, `int`     | `f64`,`f32`,`u8`-`u64`,`i8`-`i64`
| `float-32`          | `f64` or `f32`
| `float-64`          | `f64` or `f32`
| `str`               | `str`, `enum variant`, `field name`
| `bin`               | `bytes` (`&[u8]`, `Vec<u8>` with `std` or `alloc`)
| `array`             | `array`,`tuple`,`tuple struct`,`typle variant`,`seq-like`,`struct`
| `map`               | `enum variant`,`struct variant`,`map-like`,`struct`
| `T`                 | `NewType(T)`, `Some(T)`
| `fixext`, `ext`     | Unsupported

[`Serializer`]: ser::CompactSerializer
[`Deserializer`]: de::Deserializer
*/
#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "std")]
extern crate std;

#[cfg(all(feature = "alloc",not(feature = "std")))]
extern crate alloc;

pub mod ser;
pub mod de;

pub use ser_write;
pub use ser_write::SerWrite;

pub use ser::{
    to_writer_compact,
    to_writer,
    to_writer_named
};

pub use de::{
    from_slice,
    from_slice_split_tail
};

mod magick {
    use core::ops::RangeInclusive;
    /* MessagePack MAGICK */
    pub const MIN_POSFIXINT: u8 = 0x00;
    pub const MAX_POSFIXINT: u8 = 0x7f;
    pub const NEGFIXINT: u8 = 0b11100000;
    pub const MIN_NEGFIXINT: i8 = NEGFIXINT as i8; //-32
    pub const FIXINT_I16: RangeInclusive<i16> = MIN_NEGFIXINT as i16..=MAX_POSFIXINT as i16;
    pub const FIXINT_I32: RangeInclusive<i32> = MIN_NEGFIXINT as i32..=MAX_POSFIXINT as i32;
    pub const FIXINT_I64: RangeInclusive<i64> = MIN_NEGFIXINT as i64..=MAX_POSFIXINT as i64;
    pub const NIL: u8      = 0xc0;
    pub const RESERVED: u8 = 0xc1;
    pub const FALSE: u8    = 0xc2;
    pub const TRUE: u8     = 0xc3;

    pub const FIXMAP: u8   = 0x80; /* 1000xxxx */
    pub const MAX_FIXMAP_SIZE: usize = 0b1111;
    pub const FIXMAP_MAX: u8 = FIXMAP + MAX_FIXMAP_SIZE as u8; /* 10001111 */

    pub const FIXARRAY: u8 = 0x90; /* 1001xxxx */
    pub const MAX_FIXARRAY_SIZE: usize = 0b1111;
    pub const FIXARRAY_MAX: u8 = FIXARRAY + MAX_FIXARRAY_SIZE as u8; /* 10011111 */

    pub const FIXSTR: u8   = 0xa0; /* 101xxxxx */
    pub const MAX_FIXSTR_SIZE: usize = 0b11111;
    pub const FIXSTR_MAX: u8 = FIXSTR + MAX_FIXSTR_SIZE as u8; /* 10111111 */

    pub const BIN_8: u8     = 0xc4;
    pub const BIN_16: u8    = 0xc5;
    pub const BIN_32: u8    = 0xc6;

    pub const EXT_8: u8     = 0xc7;
    pub const EXT_16: u8    = 0xc8;
    pub const EXT_32: u8    = 0xc9;

    pub const FLOAT_32: u8  = 0xca;
    pub const FLOAT_64: u8  = 0xcb;

    pub const UINT_8: u8    = 0xcc;
    pub const UINT_16: u8   = 0xcd;
    pub const UINT_32: u8   = 0xce;
    pub const UINT_64: u8   = 0xcf;

    pub const INT_8: u8     = 0xd0;
    pub const INT_16: u8    = 0xd1;
    pub const INT_32: u8    = 0xd2;
    pub const INT_64: u8    = 0xd3;

    pub const FIXEXT_1: u8  = 0xd4;
    pub const FIXEXT_2: u8  = 0xd5;
    pub const FIXEXT_4: u8  = 0xd6;
    pub const FIXEXT_8: u8  = 0xd7;
    pub const FIXEXT_16: u8 = 0xd8;

    pub const STR_8: u8     = 0xd9;
    pub const STR_16: u8    = 0xda;
    pub const STR_32: u8    = 0xdb;

    pub const ARRAY_16: u8  = 0xdc;
    pub const ARRAY_32: u8  = 0xdd;

    pub const MAP_16: u8    = 0xde;
    pub const MAP_32: u8    = 0xdf;
}
