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

1. Install Microsoft Visual C++

2. Install MSYS2 - a distribution of core GNU utilities for Windows. Required for dependencies that have unixy build systems with configure scripts and such.

    https://www.msys2.org/

    Mixlab also requires a few additional packages from pacman, MSYS2's package manager. In an MSYS2 bash session, run:

    ```sh-session
    $ pacman -S make nasm pkgconfig
    ```

3. Open "x64 Native Tools Command Prompt" from the Start Menu, and type:

    ```
    > C:\msys64\msys2_shell.cmd -mingw64 -full-path
    ```

    A bash shell will appear. You will use this bash shell for the rest of the instructions and for building Mixlab.

4. Rename `/usr/bin/link.exe` to `/usr/bin/link2.exe`.

    This is pretty gross but as far as I can tell seems to be required to prevent MSYS2's `link.exe` from clashing with MSVC's. See the "Gotchas" section of http://anadoxin.org/blog/bringing-visual-studio-compiler-into-msys2-environment.html for more information.

5. Set the `CC` environment variable to `cl`

    ```sh-session
    $ export CC=cl
    ```

Then run `make` as described in the Unices section above.

## Running

Running the `mixlab` binary starts an HTTP server on `localhost:8000` serving the web UI
