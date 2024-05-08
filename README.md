ser-write
=========

Writer-style serde serializers and deserializers for convenience designed with embedded (no-std) targets in mind.

* Writer-style serializers use the common writer trait.
* Designed for `no-std`.
* Fully supports `std` or `alloc` when enabled for code portability and testablilty.
* For each serializer a deserializer is provided for convenience.
* Embedded projects can implement `SerWrite` trait for their own custom containers, frame builders and more.

This crate provides the trait - `SerWrite` which should be used by serializers.

On the other end, embedded projects can implement `SerWrite` for their own exotic containers.

Depending on the selected features, `SerWrite` is implemented for:

* `SliceWriter` - example slice writer implementation,
* [`arrayvec::ArrayVec<u8,CAP>`](https://crates.io/crates/arrayvec) - `arrayvec` feature,
* [`heapless::Vec<u8,CAP>`](https://crates.io/crates/heapless) - `heapless` feature,
* `Vec<u8>` - `alloc` or `std` feature,
* `VecDeque<u8>` - `alloc` or `std` feature,
* `io::Cursor<T: io::Write>` - `std` feature,

`std` and `alloc` features are here to help testing and porting code in different environments.


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


Alternatives
------------

For `alloc` only:
* [serde_json](https://crates.io/crates/serde_json)

Alternatively there's a Rust Embedded Community crate for serializeing JSONs without `std`:

* [serde-json-core](https://crates.io/crates/serde-json-core)
* [serde-json-core-fmt](https://crates.io/crates/serde-json-core-fmt) (a writer-style attempt abusing `fmt::Display` trait)

`serde-json-core` is a true `no_std`, no `alloc` alternative but rather inconvenient. One has to serialize data into intermediate slices instead just pushing data to outgoing buffers or frame builder implementations.


Why though?
-----------

* This crate would not be needed once something like `io::Write` lands in the Rust core.
