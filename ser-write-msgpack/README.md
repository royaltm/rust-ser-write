ser-write-msgpack
=================

MessagePack serializer for [Serde](https://crates.io/crates/serde) with `SerWrite`.

Features:

* `std` enables std library,
* `alloc` enables alloc library,

With `alloc` or `std` feature enabled `serde::ser::Serializer::collect_str` method is implemented using intermediate `String`.

Otherwise `Serializer::collect_str` is implemented by formatting a string twice, once to count the string size and the second time to actually write it.

There are 2 serializers available:

* `CompactSerializer`, `fn to_writer` - serializes structs to arrays and enum variants indexes
* `VerboseSerializer`, `fn to_writer_named` - serializes structs to maps with field names and enum variants to strings
