.PHONY: build run check

build:
	./build-frontend.sh && cargo build

run:
	./build-frontend.sh && cargo run

check:
	(cd frontend && cargo check) && cargo check
