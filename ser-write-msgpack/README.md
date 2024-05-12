ser-write-msgpack
=================

This crate provides a `no_std` friendly [MessagePack](https://msgpack.org) serializers for [serde](https://crates.io/crates/serde) using [`SerWrite`] as a writer and a deserializer for convenience.


Usage
-----

```
[dependencies]
ser-write-msgpack = { version = "0.1", default-features = false }
```


Serializer
----------

`ser-write-msgpack` comes with 3 serializers:

* `to_writer_compact` - serializes structs to arrays and enum variants as indexes,
* `to_writer` - serializes structs to maps with fields and enum variants as indexes,
* `to_writer_named` - serializes structs to maps with field names and enum variants as strings.

Features:

* `std` enables std library,
* `alloc` enables alloc library,

With `alloc` or `std` feature enabled `serde::ser::Serializer::collect_str` method is implemented using intermediate `String`.

Otherwise `Serializer::collect_str` is implemented by formatting a string twice, once to count the string size and the second time to actually write it.


Deserializer
------------

The MessagePack deserializer expects a MessagePack encoded slice of bytes. `&str` or `&[u8]` types deserialize using (ZERO-COPY) references from the provided slice.

* `from_slice` - deserializes MessagePack data from a slice of bytes
* `from_slice_split_tail` - deserializes MessagePack data from a slice of bytes returning a remaining portion of the input slice

Deserializer supports self-describing formats.

Deserializer deserializes structs from both maps and arrays using either strings or indexes as variant or field identifiers.

[`SerWrite`]: https://docs.rs/ser-write/latest/ser_write/trait.SerWrite.html