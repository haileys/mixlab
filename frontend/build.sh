#!/bin/bash

cd "$(dirname "$0")"

if [[ "$1" == "--release" ]]; then
    BUILD_MODE="--release"
else
    BUILD_MODE="--dev"
fi

wasm-pack build "$BUILD_MODE" --target no-modules
