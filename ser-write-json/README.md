ser-write-json
==============

JSON serializer for [serde](https://crates.io/crates/serde) using `SerWrite` as a writer.

Deriving from [serde-json-core](https://crates.io/crates/serde-json-core) and [serde_json](https://crates.io/crates/serde_json).


Features:

* `std` enables std library,
* `alloc` enables alloc library,

With any of the above features enabled additional `to_string...`  methods are provided for convenience.

`ser-write-json` features JSON serializers in 3 flavors. The difference between them is how they serialize byte arrays:

* `SerializerByteArray` - as a number array,
* `SerializerByteHexStr` - as a hex string,
* `SerializerBytePass` - passing through bytes to a writer assuming they contain pre-serialized JSON fragments.

All the serializers produce compact JSONs.
