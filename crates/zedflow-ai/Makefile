.PHONY: generate-models generate-image-models generate fmt check test doc package clean

generate-models:
	cargo run -p zedflow-ai --bin generate-models

generate-image-models:
	cargo run -p zedflow-ai --bin generate-image-models

generate: generate-models generate-image-models

fmt:
	cargo fmt --package zedflow-ai --check

check:
	cargo check -p zedflow-ai --all-targets

test:
	cargo test -p zedflow-ai --all-targets

doc:
	RUSTDOCFLAGS="-D warnings" cargo doc -p zedflow-ai --no-deps

package:
	cargo package -p zedflow-ai --list

clean:
	cargo clean -p zedflow-ai
