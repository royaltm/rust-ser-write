ser-write
=========

Streaming-style serde serializers for embedded (no-std) and more.

In most cases for the Rust `std` you should probably use:

* [serde_json](https://crates.io/crates/serde_json)

Alternatively there's a Rust Embedded Community crate for serializeing JSONs without `std`:

* [serde-json-core](https://crates.io/crates/serde-json-core)

While both solutions are great for `std`, with bare-metal we either have to rely on `alloc` or use `serde-json-core` serialize to a slice.

What's missing here is a functionality of `serde_json::to_writer` with both `std`  library and `no-std`.

For example I have to construct frames for a particular protocol which modifies raw bytes in a specific way.
With `serde-json-core` I have to serialize data first to an intermediate container.

What I'd prefer instead is the ability to serialize data in a streaming fashion to a custom frame container using a trait like `std::io::Write`.

There are some efforts to bring `io::Write` to `core` or at least provide something similar, but until then, there's really no good solution available.

Enter `ser-write`.

This crate provides the trait - `SerWrite` which should be used by serializers implementing `serde::ser::Serializers`.

On the other end, projects can implement `SerWrite` for their own exotic containers.

Depending on the selected features, `SerWrite` is implemented for:

* `SliceWriter` - example slice writer implementation,
* [`arrayvec::ArrayVec<u8,CAP>`](https://crates.io/crates/arrayvec) - `arrayvec` feature,
* [`heapless::Vec<u8,CAP>`](https://crates.io/crates/heapless) - `heapless` feature,
* `Vec<u8>` - `alloc` or `std` feature,
* `VecDeque<u8>` - `alloc` or `std` feature,
* `io::Cursor<T: io::Write>` - `std` feature,

`std` and `alloc` features are here to help testing and reusing code in different environments.

Serializers
-----------

Currently available serializers are:

* JSON (compact) - [ser-write-json](ser-write-json/)
* MessagePack - [ser-write-msgpack](ser-write-msgpack/)

Example
-------

An example `SliceWriter` implementation:

```rs
use ser_write::{SerWrite, SerResult, SerError};

#[derive(Debug, PartialEq)]
pub struct SliceWriter<'a> {
    pub buf: &'a mut [u8],
    pub len: usize
}

impl SerWrite for SliceWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> SerResult<()> {
        let end = self.len + buf.len();
        match self.buf.get_mut(self.len..end) {
            Some(chunk) => {
                chunk.copy_from_slice(buf);
                self.len = end;
                Ok(())
            }
            None => Err(SerError::BufferFull)
        }
    }
}
```
