[![Mysz Icon](https://raw.githubusercontent.com/mysz-lang/.github/main/images/mysz_logo_1x.jpg)](https://github.com/mysz-lang/)

# nibble

[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org)
[![Mysz Backend](https://img.shields.io/badge/backend-mysz--core-blue.svg)](https://github.com/mysz-lang/mysz-core)

`nibble` is the CLI driver for the **Mysz** compiler toolchain. It wraps `mysz-core` (which uses Cranelift for codegen) and handles everything you need to go from source to a running binary: reading your source files, keeping intermediate build artifacts safely out of the way, wiring up the ABI, and calling out to your system linker.

---

## What it does

* **One command, full pipeline**: `nibble` handles codegen, assembly, and linking so you don't have to chain together a bunch of separate tools.
* **Sandboxed builds**: intermediate `.o` files and runtime source get generated in a temp directory, so a failed build won't leave junk behind in your project.
* **Built-in runtime**: a small C runtime ships inside the `nibble` binary itself, giving you things like I/O and string concatenation (that's what powers the `+` operator) without needing anything external.
* **`--noruntime` for bare-metal work**: strips out the standard runtime entirely if you're targeting embedded systems or something with its own custom runtime.
* **`--link` for pulling in dependencies**: pass in `.c`, `.o`, `.a`, `.so`, or `.lib` files directly and `nibble` will fold them into the final link step.

---

## Before you start

`nibble` doesn't do its own linking, it hands that off to a compiler already on your machine. You'll need one of:

* **Linux / macOS:** `cc`, `gcc`, or `clang`
* **Windows:** `clang` (via LLVM) or MSVC build tools

---

## Installing

```bash
git clone https://github.com/mysz-lang/nibble.git
cd nibble
cargo build --release

# then either:
cargo install --path .
# or manually drop the binary somewhere on your PATH:
install -m 755 ./target/release/nibble {dir_on_path}
```

---

## Using it

```
The mouse-y compiler driver for the Mysz programming languag.

Usage: nibble <COMMAND>

Commands:
  build  
  run    
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

---

## Runtime

Nibble uses the standard runtime library for mysz: [mysz-lang/mysz-runtime](https://github.com/mysz-lang/mysz-runtime/)

It also has an embedded repository able to fetch the standard library for mysz: [mysz-lang/mysz-std](https://github.com/mysz-lang/mysz-std)