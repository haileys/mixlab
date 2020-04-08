# Mixlab

Digital audio workstation. AGPLv3.

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

``` sh-session
$ .\Build-Project.ps1 [-Build] [-Release]  # Build (optionally in release mode)
$ .\Build-Project.ps1 -Check [-Release]    # Check project (optionally in release mode)
$ .\Build-Project.ps1 -Run [-Release]      # Build frontend and run backend (optionally in release mode)
```

## Running

Running the `mixlab` binary starts an HTTP server on `localhost:8000` serving the web UI
