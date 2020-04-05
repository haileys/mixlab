.PHONY: build release run check

build:
	./frontend-exec.sh ./build.sh && cargo build

release:
	./frontend-exec.sh ./build.sh --release && cargo build --release

run:
	./frontend-exec.sh ./build.sh && cargo run

check:
	./frontend-exec.sh cargo check --target=wasm32-unknown-unknown && cargo check
