//! JSON serializers for ser-write
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "std")]
extern crate std;

#[cfg(all(feature = "alloc",not(feature = "std")))]
extern crate alloc;

pub mod base64;
pub mod ser;
pub mod de;
pub use de::*;

pub use ser_write::{SerWrite, SerError, SerResult};

#[cfg(any(feature = "std", feature = "alloc"))]
pub use ser::{to_string, to_string_hex_bytes, to_string_b64_bytes, to_string_pass_bytes};
pub use ser::{to_writer, to_writer_hex_bytes, to_writer_b64_bytes, to_writer_pass_bytes};
