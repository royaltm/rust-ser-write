ser-write-msgpack
=================

[![Crate][Crate img]][Crate Link]
[![Docs][Docs img]][Docs Link]
[![Build Status][Build img]][Build Link]
[![Coverage Status][Coverage img]][Coverage Link]
[![Minimum rustc version][rustc version img]][rustc version link]

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


Rust Version Requirements
-------------------------

`ser-write-msgpack` requires Rustc version 1.75 or greater.

[`SerWrite`]: https://docs.rs/ser-write/latest/ser_write/trait.SerWrite.html
[Crate Link]: https://crates.io/crates/ser-write-msgpack
[Crate img]: https://img.shields.io/crates/v/ser-write-msgpack.svg
[Docs Link]: https://docs.rs/ser-write-msgpack
[Docs img]: https://docs.rs/ser-write-msgpack/badge.svg
[Build Link]: https://github.com/royaltm/rust-ser-write/actions/workflows/rust.yml
[Build img]: https://github.com/royaltm/rust-ser-write/actions/workflows/rust.yml/badge.svg?branch=main
[rustc version link]: https://github.com/royaltm/rust-ser-write/tree/main/ser-write-msgpack#rust-version-requirements
[rustc version img]: https://img.shields.io/badge/rustc-1.75+-lightgray.svg
[Coverage Link]: https://coveralls.io/github/royaltm/rust-ser-write?branch=main
[Coverage img]: https://coveralls.io/repos/github/royaltm/rust-ser-write/badge.svg?branch=main