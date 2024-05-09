ser-write-json
==============

This crate provides a `no_std` compact JSON serializer for [serde](https://crates.io/crates/serde) using `SerWrite` as a writer and a JSON deserializer for convenience.

This crate has been in some parts derived from work of [serde-json-core](https://crates.io/crates/serde-json-core) and [serde_json](https://crates.io/crates/serde_json).


Serializer
----------

The crate provides JSON serializers in 4 flavors depending on how do you want to handle types serialized with `serialize_bytes` method.

* `to_writer` - serialize bytes as number arrays,
* `to_writer_hex_bytes` - as HEX-encoded strings,
* `to_writer_base64_bytes` - as Base64 encoded strings,
* `to_writer_pass_bytes` - passing through bytes to a writer assuming they contain pre-serialized JSON fragments.
* `to_writer_with_encoder` - a custom encoder can be provided.

Custom string encoders can be implemented using `ByteEncoder` trait. There's an [example][examples/] in this repository that does exactly that.

Features:

* `std` enables std library,
* `alloc` enables alloc library

With any of the above features enabled additional `to_string...`  methods are provided for convenience.


Deserializer
------------

The JSON deserializer expects a JSON encoded mutable slice of bytes. `&str` or `&[u8]` types deserialize using (ZERO-COPY) references from the provided slice. The slice needs to be mutable so the decoder can unescape JSON strings and decode bytes from strings in various formats in-place.

The JSON deserializer is available in 4 flavors depending on how do you want to handle types deserialized with `deserialize_bytes` method from JSON strings:

* `from_mut_slice` - decodes bytes from regular JSON strings without checking if they are proper UTF-8 strings,
* `from_mut_slice_hex_bytes` - expect two hexadecimal ASCII characters per byte,
* `from_mut_slice_base64_bytes` - expect Base64 encoded string,
* `from_mut_slice_with_decoder` - a custom decoder can be provided.

Deserializer can also deserialize bytes in-place from a JSON array of numbers regardless of the chosen implementation.

Deserializer supports self-describing formats.

Deserializer deserializes structs from both JSON objects or arrays.
