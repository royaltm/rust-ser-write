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
