#!/bin/bash

cd "$(dirname "$0")/frontend"

if [[ "$1" == "--release" ]]; then
    BUILD_MODE="--release"
else
    BUILD_MODE="--dev"
fi

export RUSTFLAGS="--remap-path-prefix src=frontend/src"
wasm-pack build "$BUILD_MODE" --target no-modules
