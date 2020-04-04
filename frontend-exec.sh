#!/bin/bash

cd "$(dirname "$0")/frontend"
export RUSTFLAGS="--remap-path-prefix src=frontend/src"
exec "$@"
