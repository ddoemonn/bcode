# Contributing to bcode

PRs are welcome. Here's how to get started.

## Setup

```bash
git clone https://github.com/ddoemonn/bcode.git
cd bcode
cargo build
```

Requires Rust stable (1.75+).

## Running locally

```bash
cargo run -- --provider ollama --model llama3.2
```

Or with an API key:

```bash
ANTHROPIC_API_KEY=sk-ant-... cargo run
```

## Code style

- No clippy warnings — run `cargo clippy` before submitting
- No comments that describe what code does — only comments that explain *why*
- Conventional commits: `feat:`, `fix:`, `refactor:`, `docs:`
- Keep modules focused — if a file exceeds ~300 lines, split it

## Adding a provider

1. Create `src/provider/myprovider.rs` implementing the `Provider` trait
2. Add it to `src/provider/mod.rs`
3. Add an entry in `PROVIDERS` in `src/app/mod.rs`
4. Handle it in `try_build_provider` in `src/main.rs`
5. Handle it in `make_provider` in `src/app/mod.rs`

## Adding a tool

1. Add the tool struct in `src/tools/fs.rs` or `src/tools/shell.rs` (or a new file)
2. Register it in the `registry()` function in `src/tools/mod.rs`

## Reporting bugs

Open a GitHub issue with:
- OS and terminal emulator
- `bcode` version (`bcode --version`)
- Steps to reproduce
- Expected vs actual behavior
