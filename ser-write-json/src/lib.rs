//! A JSON (compact) serde serializer for [`ser-write`](`ser_write`) and a JSON deserializer for convenience.
/*!

[`Serializer`] types:

| Serde type ->     | JSON type
|-------------------|--------------------
| `()`              | `null`
| `Unit` struct     | `null`
| `bool`            | `boolean`
| `NewType(T)`      | `T` -> `JSON`
| `None`            | `null`
| `Some(T)`         | `T` -> `JSON`
| `u8`-`u64`        | `number`
| `i8`-`i64`        | `number`
| `f23`,`f64`       | `number`
| `str`             | `string`
| `bytes`           | (configurable)
| `array`, `tuple`  | `array`
| `seq`-like        | `array`
| `map`-like        | `object`
| `struct`          | `object`
| `unit variant`    | `string`
| `newtype variant` | `{"Name":T -> JSON}`
| `tuple variant`   | `{"Name": array}`
| `struct variant`  | `{"Name": object}`

[`Deserializer`] supports self-describing formats (`deserialize_any`).

[`Deserializer`] deserializes structs from both JSON objects or arrays.

[`Deserializer`] types:

| JSON type ->      | Serde type (depending on context)
|-------------------|----------------------------------------
| `null`            | `unit`,`none`,`NaN`
| `boolean`         | `bool`
| `number`          | `f64`,`f32`,`u8`-`u64`,`i8`-`i64`
| `string`          | `str`,`bytes` (configurable),`enum variant`
| `array`           | `array`,`tuple`,`tuple struct`,`typle variant`,`seq-like`,`struct`
| `object`          | `enum variant`,`struct variant`,`map-like`,`struct`
| `T`               | `NewType(JSON -> T)`, `Some(JSON -> T)`

[`Serializer`]: ser::Serializer
[`Deserializer`]: de::Deserializer
*/
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
