name := 'ser-write'

embedded:
    cargo build -p ser-write-json-embedded-example --target thumbv7em-none-eabihf
    cargo build -p ser-write-msgpack-embedded-example --target thumbv7em-none-eabihf

example:
    cargo run --example custom_bytes --release
    cargo run --example serde --release

# build all docs
doc:
    RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --all-features

# run all tests
test:
    cargo test --no-default-features -- --nocapture --test-threads=1
    cargo test --no-default-features --features=alloc -- --nocapture --test-threads=1
    cargo test -- --nocapture --test-threads=1
    cargo test -r --no-default-features -- --nocapture --test-threads=1
    cargo test -r --no-default-features --features=alloc -- --nocapture --test-threads=1
    cargo test -r -- --nocapture --test-threads=1
    cargo test -p ser-write --no-default-features --features=arrayvec,heapless,tinyvec -- --nocapture --test-threads=1
    cargo test -p ser-write --all-features -- --nocapture --test-threads=1
    cargo test -p ser-write-json --features=de-any-f32 -- --nocapture --test-threads=1
    cargo test -p ser-write-json --no-default-features --features=de-any-f32 -- --nocapture --test-threads=1
    cargo test -p ser-write-json --no-default-features --features alloc,de-any-f32 -- --nocapture --test-threads=1

# run clippy tests
clippy: clippy-json clippy-mp
    touch src/lib.rs
    cargo clippy -- -D warnings
    cargo clippy --all-features -- -D warnings
    cargo clippy --no-default-features -- -D warnings

# run clippy tests ser-write-json
clippy-json:
    touch ser-write-json/src/lib.rs
    cargo clippy -p ser-write-json -- -D warnings
    cargo clippy -p ser-write-json --features=de-any-f32 -- -D warnings
    cargo clippy -p ser-write-json --no-default-features --features=alloc -- -D warnings

# run clippy tests ser-write-msgpack
clippy-mp:
    touch ser-write-msgpack/src/lib.rs
    cargo clippy -p ser-write-msgpack -- -D warnings
    cargo clippy -p ser-write-msgpack --no-default-features --features=alloc -- -D warnings

# report coverage locally
cov:
    cargo llvm-cov clean --workspace
    cargo llvm-cov --no-report
    cargo llvm-cov --no-report --all-features
    cargo llvm-cov --no-report --no-default-features --features=alloc
    cargo llvm-cov --no-report --no-default-features
    cargo llvm-cov report --lcov --output-path=lcov.info
    cargo llvm-cov report --html --open

clean:
    cargo clean
