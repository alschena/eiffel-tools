# Eiffel-tools

This project introduces `lsp-eiffel`, a server-side implementation of the [language server protocol](https://microsoft.github.io/language-server-protocol/).

## Usage

### Language server

Configure your favorite editor with LSP support to use the language server.
(Make a pull request with the specific instruction for your code editor e.g. IntelliJ, VSCode, Emacs, Neovim, Zed, Helix)

### Helix

```toml
# ${HOME}/.config/helix/languages.toml
[language-server.lsp-eiffel]
command = "placeholder" # Change `placeholder` with the path to the language server binary.

[[language]]
name = "eiffel"
scope = "source.eiffel"
file-types = ["e"]
comment-token = "--"
roots = []
language-servers = ["lsp-eiffel"]

[[grammar]]
name = "eiffel"
source = { git = "https://github.com/imustafin/tree-sitter-eiffel.git", rev = "44c4c5cf912abb2a3cca04546fbca28fd4b3cecb" }
```

### Standalone commands

The commands `llm-correct-classes` and `llm-correct-features` require specific inputs.
First, you need to provide the path to the Eiffel project configuration file (ecf) after the `--config` option.
Additionally, after the `--classes` option, you must specify the path to a file that contains a list of class names, with one name per line, that you wish to correct.

## Installation

### Dependencies

You need a [Constructor tech API token](https://docs.constructor.tech/articles/#!model-developer-guide/input) to use the LLMs.
You need access to [AutoProof](https://se.constructor.ch/reif-site/autoproof) binaries to use the verifier.

### Environment

This project needs the following environmental variables correctly set.
The variable ` CONSTRUCTOR_APP_API_TOKEN` must point to a constructor knowledge access token.
The variable `AP_COMMAND` must point to the AutoProof executable.

### Cargo

This project is built using [cargo](https://doc.rust-lang.org/cargo/index.html).
If you are new to cargo and rust, start from the [official guide](https://www.rust-lang.org/tools/install).

The command `cargo test` tests the current crate.
The command `cargo test --workspace` runs all the tests in the workspace.

The command `cargo build --release --workspace` builds the language server and the standalone commands.
The command `cargo build --release -p eiffel-tools` builds only the language server.
The command `cargo build --workspace` builds the language server and the standalone commands in the workspace with debugging information e.g. bounds checks.

