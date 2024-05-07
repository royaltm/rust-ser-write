//! JSON serializers for ser-write
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "std")]
extern crate std;

#[cfg(all(feature = "alloc",not(feature = "std")))]
extern crate alloc;

pub mod ser;
pub mod de;
pub use ser::*;
pub use de::*;
