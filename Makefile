.PHONY: build run check

build:
	./frontend-exec.sh ./build.sh && cargo build

run:
	./frontend-exec.sh ./build.sh && cargo run

check:
	./frontend-exec.sh cargo check && cargo check
