# Mixlab

Digital audio/video workstation. AGPLv3.

## Building

Make sure you have wasm-pack installed first: `cargo install wasm-pack`

### Unices

``` sh-session
$ make [build]   # build frontend and backend
$ make release   # build frontend and backend in release mode
$ make check     # check frontend and backend
$ make run       # build frontend and run backend
```

### Windows

Requirements:

* **MinGW + MSYS**

    Core GNU utilities for Windows. Required for dependencies that rely on tools such as `tar` or `make` in their build scripts.

    http://www.mingw.org/wiki/getting_started

* **LLVM**

    Some dependencies generate bindings with bindgen on the fly in their build scripts. bindgen relies on `libclang.dll` from LLVM.

    See https://github.com/rust-lang/rust-bindgen/blob/master/book/src/requirements.md for install instructions

``` sh-session
$ .\Build-Project.ps1 [-Build] [-Release]  # Build (optionally in release mode)
$ .\Build-Project.ps1 -Check [-Release]    # Check project (optionally in release mode)
$ .\Build-Project.ps1 -Run [-Release]      # Build frontend and run backend (optionally in release mode)
```

## Running

Running the `mixlab` binary starts an HTTP server on `localhost:8000` serving the web UI
