
build-mac:
	cargo build -p ts-client --target x86_64-apple-darwin --release
	cbindgen --config ts-client/cbindgen.toml ts-client/src/lib.rs > target/x86_64-apple-darwin/release/ts.h
	open target/x86_64-apple-darwin/release

build-ios:
	cargo build -p ts-client --target aarch64-apple-ios --release
	cbindgen --config ts-client/cbindgen.toml ts-client/src/lib.rs > target/aarch64-apple-ios/release/ts.h
	open target/aarch64-apple-ios/release

ios:
	cargo build --release --target aarch64-apple-ios --manifest-path leaf-ffi/Cargo.toml --no-default-features --features "default-openssl"
	cbindgen --config leaf-ffi/cbindgen.toml leaf-ffi/src/lib.rs > target/aarch64-apple-ios/release/leaf.h

lib:
	cargo build -p leaf-ffi --release
	cbindgen --config leaf-ffi/cbindgen.toml leaf-ffi/src/lib.rs > target/release/leaf.h

lib-dev:
	cargo build -p leaf-ffi
	cbindgen --config leaf-ffi/cbindgen.toml leaf-ffi/src/lib.rs > target/debug/leaf.h

local:
	cargo build -p leaf-bin --release

local-dev:
	cargo build -p leaf-bin

mipsel:
	./misc/build_cross.sh mipsel-unknown-linux-musl

mips:
	./misc/build_cross.sh mips-unknown-linux-musl

test:
	cargo test -p leaf -- --nocapture

# Force a re-generation of protobuf files.
proto-gen:
	touch leaf/build.rs
	PROTO_GEN=1 cargo build -p leaf
