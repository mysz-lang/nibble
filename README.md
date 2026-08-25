[![Mysz Icon](https://raw.githubusercontent.com/mysz-lang/.github/main/images/mysz_logo_1x.jpg)](https://github.com/mysz-lang/)

# nibble

[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](https://www.rust-lang.org)
[![Mysz Backend](https://img.shields.io/badge/backend-mysz--core-blue.svg)](https://github.com/mysz-lang/mysz-core)

`nibble` is the command-line tool and package manager for the **Mysz** programming language.

It uses `mysz-core` to compile Mysz programs, manages project dependencies, and links the final program into a native executable.

---

## Features

- Build Mysz programs into native executables.
- Run Mysz programs directly.
- Check Mysz source without building it.
- Choose between the **Cranelift** and **LLVM** compiler backends.
- Configure compiler settings in `nibble.toml`.
- Download and manage packages.
- Use packages from remote `.tar.gz` archives.
- Automatically download the Mysz runtime when needed.
- Link extra native files with `--link`.
- Build without the runtime with `--noruntime`.
- Create new projects with `nibble init`.

---

## Requirements

Nibble needs a native C compiler for the final linking step.

- **Linux / macOS:** `cc`, `gcc`, or `clang`
- **Windows:** `clang` or MSVC Build Tools

---

## Installation

Clone and build Nibble:

```bash
git clone https://github.com/mysz-lang/nibble.git
cd nibble
cargo build --release
```

Install with Cargo:

```bash
cargo install --path .
```

---

## Creating a Project

Create a new project:

```bash
nibble init my-project
```

This creates:

```text
my-project/
-- main.mysz
-- nibble.toml
```

You can also create a project in the current directory:

```bash
nibble init
```

---

## nibble.toml

`nibble.toml` stores your project settings and dependencies.

A simple project looks like this:

```toml
[compiler]
target = "cranelift"
output_json = false

[dependencies]
std = "std"
```

### Compiler Backend

You can choose which `mysz-core` backend to use.

For Cranelift:

```toml
[compiler]
target = "cranelift"
```

For LLVM:

```toml
[compiler]
target = "llvm"
```

This setting is used by `build`, `run`, and `check`.

Cranelift is used by default if no backend is specified.

* check compatibility between backend and your machine [here](https://github.com/mysz-lang/mysz-core#support)

### JSON Output

You can also enable JSON compiler output:

```toml
[compiler]
target = "cranelift"
output_json = true
```

This is useful for tools such as editors.

---

## Dependencies

Dependencies are listed in `nibble.toml`:

```toml
[dependencies]
std = "std"
```

Nibble downloads missing dependencies automatically when building a project.

Packages are stored in:

```text
~/.nibble/packs/
```

### Custom Dependencies

You can also use a package from a remote archive:

```toml
[dependencies]
my_library = {
    source = "https://example.com/my-library.tar.gz",
    root_dir = "src"
}
```

- `source` is the URL of the archive.
- `root_dir` is the folder inside the archive containing the Mysz files.
- `archive_prefix` can be used when Nibble cannot detect the archive's top-level folder.

For example:

```toml
[dependencies]
my_library = {
    source = "https://example.com/my-library.tar.gz",
    root_dir = "src",
    archive_prefix = "my-library"
}
```

---

## Building

Build a program:

```bash
nibble build main.mysz
```

This creates a program called `main`.

Choose the output name:

```bash
nibble build main.mysz -o my_program
```

Build multiple files:

```bash
nibble build main.mysz other.mysz -o my_program
```

### Optimization

Use `-O` to enable optimization:

```bash
nibble build main.mysz -O
```

### No Runtime

Use `--noruntime` if you do not want Nibble to link the Mysz runtime:

```bash
nibble build main.mysz --noruntime
```

### Extra Files

Use `--link` to pass extra files to the native linker:

```bash
nibble build main.mysz --link helper.o
```

You can pass more than one:

```bash
nibble build main.mysz \
    --link helper.o \
    --link library.a
```

---

## Include Paths

Add extra source directories with `-I`:

```bash
nibble build main.mysz -I ./libs
```

You can add multiple directories:

```bash
nibble build main.mysz \
    -I ./libs \
    -I ./vendor
```

Nibble also searches the package directory:

```text
~/.nibble/packs/
```

If no include paths are given, Nibble can also use the `NIBBLE_PATH` environment variable.

---

## Running

Run a Mysz program:

```bash
nibble run main.mysz
```

Nibble builds a temporary executable, runs it, and then deletes it.

The backend from `nibble.toml` is used automatically.

---

## Checking

Check a Mysz file without building an executable:

```bash
nibble check main.mysz
```

You can also add include paths:

```bash
nibble check main.mysz -I ./libs
```

`check` uses JSON output so other tools can easily read the result.

---

## Installing Packages

Install a package manually:

```bash
nibble install std
```

Installed packages are stored in:

```text
~/.nibble/packs/
```

If the package is already installed, Nibble will not download it again.

---

## Global Packages

Nibble currently has a small built-in package registry.

The standard library can be installed with:

```bash
nibble install std
```

Or added to `nibble.toml`:

```toml
[dependencies]
std = "std"
```

### Adding a Package

Packages can be added to the registry by editing `src/packages.rs` and adding an entry to `get_default_registry()`.

Example:

```rust
registry.insert(
    "my-package",
    PackageRegistryInfo {
        tarball_url: "https://example.com/my-package.tar.gz".to_string(),
        archive_prefix: "my-package".to_string(),
        root_dir: "src".to_string(),
    },
);
```

Then submit a pull request.

---

## Runtime

Nibble automatically downloads the Mysz runtime when a normal build needs it.

The runtime is cached in:

```text
~/.nibble/cache/
```

Future builds use the cached copy.

To build without it:

```bash
nibble build main.mysz --noruntime
```

---

## Project Layout

A basic project looks like:

```text
my-project/
-- main.mysz
-- nibble.toml
```

The package cache is kept outside the project:

```text
~/.nibble/
-- cache/
-- packs/
```

---

## License

See the repository's license files for licensing information.
