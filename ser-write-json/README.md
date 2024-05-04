ser-write-json
==============

JSON serializer for [serde](https://crates.io/crates/serde) using `SerWrite` as a writer.

This crate has been in some parts derived from work of [serde-json-core](https://crates.io/crates/serde-json-core) and [serde_json](https://crates.io/crates/serde_json).

`ser-write-json` features JSON serializers in 3 flavors. The difference between them is how they implement `serialize_bytes` method:

* `SerializerByteArray`, `fn to_writer` - as a number array,
* `SerializerByteHexStr`, `fn to_writer_hex_bytes` - as a hex string,
* `SerializerBytePass`, `fn to_writer_pass_bytes` - passing through bytes to a writer assuming they contain pre-serialized JSON fragments.

All above serializers produce compact JSONs.

Features:

* `std` enables std library,
* `alloc` enables alloc library

With any of the above features enabled additional `to_string...`  methods are provided for convenience.
