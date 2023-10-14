run_dev:
	 cargo watch -q -c -s 'cargo fmt && cargo clippy && cargo run --bin runtime'
build:
	cargo build --release --bin runtime