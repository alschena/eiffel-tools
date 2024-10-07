# Installation

## Rustup

To install rust's official toolchain use `rustup`.
If you are running macOS, Linux or another Unix-like OS you can run
```curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh```

In case of doubt refer to the [official guidance](https://www.rust-lang.org/tools/install).

## Cargo

This project is built using [cargo](https://doc.rust-lang.org/cargo/index.html).

The command `cargo test` tests the current crate.
The command `cargo test --workspace` runs all the tests in the workspace. Run the latter before any commit.

# Anatomy of project

## LSP

This project introduces `lsp-eiffel`, a server-side implementation of the [language server protocol](https://microsoft.github.io/language-server-protocol/).

## Gemini

Rust wrapper around the [REST Gemini API](https://ai.google.dev/api?lang=python).

