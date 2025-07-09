# Eiffel-tools

This project introduces `lsp-eiffel`, a server-side implementation of the [language server protocol](https://microsoft.github.io/language-server-protocol/).

## Installation

### Rustup

To install rust's official toolchain use `rustup`.
If you are running macOS, Linux or another Unix-like OS you can run
```curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh```

In case of doubt refer to the [official guidance](https://www.rust-lang.org/tools/install).

### Cargo

This project is built using [cargo](https://doc.rust-lang.org/cargo/index.html).

The command `cargo test` tests the current crate.
The command `cargo test --workspace` runs all the tests in the workspace.

The command `cargo build --release --workspace` builds the language server and the standalone commands.
The command `cargo build --release -p eiffel-tools` builds only the language server.
The command `cargo build --workspace` builds the language server and the standalone commands in the workspace with debugging information e.g. bounds checks.

### Environment

This project needs the following environmental variables correctly set.
The variable ` CONSTRUCTOR_APP_API_TOKEN` must point to a constructor knowledge access token.
The variable `AP_COMMAND` must point to the AutoProof executable.

## Usage

### Language server

Build the language server.
Configure your favorite editor with LSP support (e.g. VSCode, Emacs, Neovim, Zed, Helix) to use the just-built language server.

### Standalone commands

The commands `llm-correct-classes` and `llm-correct-features` require specific inputs.
First, you need to provide the path to the Eiffel project configuration file (ecf) after the `--config` option.
Additionally, after the `--classes` option, you must specify the path to a file that contains a list of class names, with one name per line, that you wish to correct.


