[![Mysz Icon](https://raw.githubusercontent.com/mysz-lang/.github/main/images/mysz_logo_1x.jpg)](https://github.com/mysz-lang/)

# nibble

[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org)
[![Mysz Backend](https://img.shields.io/badge/backend-mysz--core-blue.svg)](https://github.com/mysz-lang/mysz-core)

`nibble` is the CLI driver and package manager for the **Mysz** programming language toolchain. It wraps `mysz-core` (which uses Cranelift for codegen) and manages the entire development lifecycle: resolving dependency manifests, fetching remote packages over the network, building intermediate compiler structures, aligning runtime libraries, and invoking your platform's native system linker to produce standalone binary builds.

---

## Features

* **One-Command Build Pipeline**: Automates parsing, codegen, object assembly, and platform linkage in a single step.
* **Declarative Package Management**: Manages project requirements through a local `nibble.toml` manifest file, automatically handling cross-module search paths.
* **Hybrid Dependency Ecosystem**: Supports global package shortcuts (like `std`) as well as custom remote HTTP/GitHub source archives (`.tar.gz`) with tailored root folder filters.
* **Sandboxed Build Isolation**: Isolates temporary `.o` frames inside standard OS temporary directories, ensuring aborted compilation passes do not clutter your workspace.
* **Automatic Target Provisioning**: Seamlessly ensures directory hierarchies exist at the destination before running final linkage passes, avoiding common missing-directory linker issues.
* **Detachable Engine Configurations**: Use `--noruntime` for bare-metal, embedded, or custom kernel architectures, or utilize `--link` to stitch external `.c`, `.o`, `.a`, `.so`, or `.lib` binaries into the executable.

---

## Prerequisites

`nibble` delegates machine-code alignment and final executable building to an existing toolchain on your host platform. Ensure one of the following is globally available:

* **Linux / macOS:** `cc`, `gcc`, or `clang`
* **Windows:** `clang` (via LLVM) or MSVC Build Tools

---

## Installation

```bash
git clone [https://github.com/mysz-lang/nibble.git](https://github.com/mysz-lang/nibble.git)
cd nibble
cargo build --release

# Install locally via cargo:
cargo install --path .

# Or manually place the binary somewhere on your path:
install -m 755 ./target/release/nibble /usr/local/bin/
```

## Dependency Management & nibble.toml

`nibble` reads a local `nibble.toml` file inside your project root to handle external dependencies. It automatically checks your cache directory (`~/.nibble/packs/`) and resolves missing dependencies right before starting a build pass.

### Manifest Schema

Create a `nibble.toml` file in the root of your project:

```toml
[dependencies]
# 1. Using a registered shortcut from the global catalog:
std = "std"

# 2. Pulling a custom module bundle directly from a remote archive URL:
custom_std = { source = "https://github.com/mysz-lang/mysz-std/archive/refs/heads/main.tar.gz", root_dir = "src" }
```

- `source`: The public URL pointing to a `.tar.gz` archive snapshot of the code library.
- `root_dir`: The directory path inside the archive containing the `.mysz source files`. `nibble` automatically extracts this specific path and drops its contents into the package namespace, keeping repository assets like readmes, tests, and manifests out of your compiler search path.

## Command Line Usage

```
Mysz •<:3O-~

Usage: nibble <COMMAND>

Commands:
  build    Compile source files and link dependencies into a native binary executable
  run      Compile and execute a Mysz script ephemerally
  install  Manually download and unpack a specific package from the global registry
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

# Global catalog repository

Currently the global catalog (an internal repository system) only contains std:

```
nibble install std
```

```toml
[dependencies]
std = "std"
```