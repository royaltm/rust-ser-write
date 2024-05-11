name := 'ser-write'

embedded:
    cargo build -p ser-write-json-embedded-example --target thumbv7em-none-eabihf

example:
    cargo run --example ser-write-json-embedded-example --release

# build all docs
doc:
    RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --all-features

# run all tests
test:
    cargo test --no-default-features -- --nocapture --test-threads=1
    cargo test --no-default-features --features=alloc -- --nocapture --test-threads=1
    cargo test -- --nocapture --test-threads=1

# run clippy tests
clippy: clippy-json clippy-mp
    touch src/lib.rs
    cargo clippy -- -D warnings
    cargo clippy --no-default-features -- -D warnings

# run clippy tests ser-write-json
clippy-json:
    touch ser-write-json/src/lib.rs
    cargo clippy -p ser-write-json -- -D warnings
    cargo clippy -p ser-write-json --no-default-features --features=alloc -- -D warnings

# run clippy tests ser-write-msgpack
clippy-mp:
    touch ser-write-msgpack/src/lib.rs
    cargo clippy -p ser-write-msgpack -- -D warnings
    cargo clippy -p ser-write-msgpack --no-default-features --features=alloc -- -D warnings

clean:
    cargo clean
