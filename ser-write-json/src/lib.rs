//! A JSON (compact) serde serializer for [`ser-write`](`ser_write`) and a deserializer for convenience.
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "std")]
extern crate std;

#[cfg(all(feature = "alloc",not(feature = "std")))]
extern crate alloc;

pub mod base64;
pub mod ser;
pub mod de;

pub use ser_write;
use ser_write::{SerWrite, SerError};

#[cfg(any(feature = "std", feature = "alloc"))]
pub use ser::{
    to_string,
    to_string_hex_bytes,
    to_string_base64_bytes,
    to_string_pass_bytes
};
pub use ser::{
    to_writer_with_encoder,
    to_writer,
    to_writer_hex_bytes,
    to_writer_base64_bytes,
    to_writer_pass_bytes
};
pub use de::{
    from_mut_slice_with_decoder,
    from_mut_slice,
    from_mut_slice_hex_bytes,
    from_mut_slice_base64_bytes
};
