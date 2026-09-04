# Ark

**Ark** is a small, extensible package manager written in Rust.

Ark keeps its core deliberately simple. Additional functionality is provided through **extensions**, which are standalone programs that can be written in **any language**.

Ark does not require extensions to use Rust, Clap, or any particular framework. It simply discovers and executes an extension, passing along the arguments provided by the user.

## Features

* 🦀 Written in Rust
* 📦 Small and lightweight package manager
* 🧩 Extensible through external programs
* 🌐 Extensions can be written in any language
* ⚡ Simple argument-based extension interface
* 🛠️ CLI powered by [`clap`](https://docs.rs/clap)
* 🎯 Minimal core with functionality provided by extensions

## Installation

> Installation instructions are coming soon.

## Usage

The basic Ark command is:

```bash
ark <command>
```

For example:

```bash
ark install <package>
ark remove <package>
ark update
```

Run:

```bash
ark --help
```

to see the commands available in your installation.

## Extensions

Extensions are a fundamental part of Ark.

An extension is simply an **executable program** that Ark can invoke. Because Ark communicates with extensions through their command-line arguments, an extension can be written in virtually any language.

For example:

```text
ark
├── install
├── remove
├── update
└── my-extension
```

When the user runs:

```bash
ark my-extension foo --bar baz
```

Ark resolves `my-extension` and executes the corresponding extension, passing the arguments through:

```text
foo --bar baz
```

The extension is responsible for interpreting those arguments and performing its functionality.

### Any language

Extensions are not tied to Rust.

You can write an Ark extension in:

* Rust
* C / C++
* Go
* Python
* JavaScript / TypeScript
* Ruby
* Java
* Shell
* Or any other language capable of producing an executable

For example, a simple shell extension could be:

```bash
#!/bin/sh

echo "Ark extension"
echo "Arguments: $@"
```

The same extension could instead be implemented as a compiled Rust binary, Python executable, Go program, or anything else.

### Simple interface

Ark's extension interface is intentionally minimal.

The core does not need to understand what an extension does. It only needs to:

1. Identify the requested extension.
2. Launch the extension.
3. Pass the user's arguments to it.
4. Return the extension's result to the user.

This keeps the extension API language-independent and allows extensions to evolve independently of Ark.

## CLI

Ark uses [`clap`](https://docs.rs/clap) for its command-line interface.

Clap handles Ark's built-in command definitions and provides the structure used to integrate extension commands into the CLI.

The important distinction is that **Clap is part of Ark's CLI implementation, not a requirement for extensions**.

An extension does not need to use Clap or even be written in Rust.

## Architecture

Ark consists of a small core surrounded by independently implemented extensions.

```text
                         ┌─────────────┐
                         │     Ark     │
                         │    (Rust)   │
                         └──────┬──────┘
                                │
                    ┌───────────┴───────────┐
                    │                       │
              Core commands          Extension runner
                    │                       │
             ┌──────┴──────┐        ┌──────┴──────┐
             │             │        │             │
          install        remove   Extension A   Extension B
                                      │             │
                                   Python         Go
```

The core does not need to know the implementation language of an extension.

```text
User
 │
 │ ark example hello --verbose
 ▼
Ark
 │
 │ executes extension
 │ arguments:
 │   hello --verbose
 ▼
Extension
 │
 ▼
Result
```

## Design Goals

### Small

Ark should provide a focused package-management core without accumulating functionality that can be implemented independently.

### Extensible

New functionality should be addable without modifying Ark itself.

### Language-independent

Extensions should not be tied to Rust or any specific runtime, framework, or SDK.

### Simple

The boundary between Ark and an extension should be as simple as possible: Ark executes a program and provides its arguments.

### Independent

Extensions can be developed, compiled, released, and maintained separately from Ark.

## Development

Clone the repository:

```bash
git clone <repository-url>
cd ark
```

Build:

```bash
cargo build
```

Run:

```bash
cargo run -- --help
```

Run tests:

```bash
cargo test
```

Format:

```bash
cargo fmt
```

Run Clippy:

```bash
cargo clippy
```

## Contributing

Contributions are welcome.

Before submitting a pull request, run:

```bash
cargo fmt
cargo clippy
cargo test
```

For larger changes, open an issue to discuss the design first.

## License

Ark is licensed under the **GNU General Public License (GPL)**.

See the `LICENSE` file for the full license text.
