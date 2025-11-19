v0.4.0
* deps: heapless 0.9.2.
* rust-version: 1.87.
* ser-write-json: remaining_len method added to the deserializer.
* ser-write-json: writer_ref method added to the serializer.
* ser-write-msgpack: remaining_len method added to the deserializer.
* ser-write-msgpack: writer_ref method added to the serializer.

v0.3.1
* deps: bump tinyvec, smallvec
* ser-write-json: publicly expose StringCollector

v0.3.0
* rust-version: 1.81.
* deps: bump serde to 1.0.210.
* ser-write: `SerError` implements `core::error::Error`.
* ser-write: heap allocating foreign implementations return `BufferFull` error instead on panicking.
* ser-write: `smallvec` and `tinyvec` support.
* ser-write-json: `de-any-f32` feature.
