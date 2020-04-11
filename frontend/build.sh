#!/bin/bash

cd "$(dirname "$0")"

if [[ "$#" == 0 ]]; then
    BUILD_MODE="--dev"
else
    BUILD_MODE="$1"
fi

echo $BUILD_MODE

wasm-pack build "$BUILD_MODE" --target no-modules
