//! MessagePack serializers for ser-write
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "std")]
extern crate std;

#[cfg(all(feature = "alloc",not(feature = "std")))]
extern crate alloc;

pub mod ser;
pub use ser::*;

mod magick {
    use core::ops::RangeInclusive;
    /* MessagePack MAGICK */
    pub const MAX_POSFIXINT: u8 = 0x7f;
    pub const MIN_NEGFIXINT: i8 = 0b11100000u8 as i8; //-32
    pub const FIXINT_I16: RangeInclusive<i16> = MIN_NEGFIXINT as i16..=MAX_POSFIXINT as i16;
    pub const FIXINT_I32: RangeInclusive<i32> = MIN_NEGFIXINT as i32..=MAX_POSFIXINT as i32;
    pub const FIXINT_I64: RangeInclusive<i64> = MIN_NEGFIXINT as i64..=MAX_POSFIXINT as i64;
    pub const NIL: u8      = 0xc0;
    pub const FALSE: u8    = 0xc2;
    pub const TRUE: u8     = 0xc3;
    pub const FIXMAP: u8   = 0x80; /* 1000xxxx */
    pub const MAX_FIXMAP_SIZE: usize = 0b1111;
    pub const FIXARRAY: u8 = 0x90; /* 1001xxxx */
    pub const MAX_FIXARRAY_SIZE: usize = 0b1111;
    pub const FIXSTR: u8   = 0xa0; /* 101xxxxx */
    pub const MAX_FIXSTR_SIZE: usize = 0b11111;
    pub const BIN_8: u8    = 0xc4;
    pub const BIN_16: u8   = 0xc5;
    pub const BIN_32: u8   = 0xc6;
    pub const FLOAT_32: u8 = 0xca;
    pub const FLOAT_64: u8 = 0xcb;
    pub const UINT_8: u8   = 0xcc;
    pub const UINT_16: u8  = 0xcd;
    pub const UINT_32: u8  = 0xce;
    pub const UINT_64: u8  = 0xcf;
    pub const INT_8: u8    = 0xd0;
    pub const INT_16: u8   = 0xd1;
    pub const INT_32: u8   = 0xd2;
    pub const INT_64: u8   = 0xd3;
    pub const STR_8: u8    = 0xd9;
    pub const STR_16: u8   = 0xda;
    pub const STR_32: u8   = 0xdb;
    pub const ARRAY_16: u8 = 0xdc;
    pub const ARRAY_32: u8 = 0xdd;
    pub const MAP_16: u8   = 0xde;
    pub const MAP_32: u8   = 0xdf;
}
