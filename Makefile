.PHONY: build profile release run check

build:
	./frontend-exec.sh ./build.sh && cargo build

profile:
	./frontend-exec.sh ./build.sh --profiling && cargo build --release

release:
	./frontend-exec.sh ./build.sh --release && cargo build --release

run:
	./frontend-exec.sh ./build.sh && cargo run workspace/

check:
	./frontend-exec.sh cargo check --target=wasm32-unknown-unknown && cargo check

.PHONY: b r c
b: build
r: run
c: check
