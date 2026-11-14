# Logic Analyzer

A logic analyzer application written in Rust, functionally comparable to PulseView.
The main goal is a program that is stable: it does not crash and does not lose data.

**Status**: early development. The foundation (workspace, CI, error model, logging,
worker panic containment) and the signal storage core (`la-storage`: transition-based
storage, fast viewport queries, bounded memory, gaps) and the device layer
(`la-devices`: device abstraction, background acquisition into the store, failure handling
that keeps captured data, an always-available seeded virtual device, and a hardware-free
test harness) exist so far; there is no UI and there are no real hardware drivers yet. See [roadmap.md](roadmap.md).

Supported operating systems: Windows 10/11 and Debian 13.

## Building and testing

Requirements:

- [`rustup`](https://rustup.rs). The repository pins the development toolchain in
  `rust-toolchain.toml`; `rustup` installs it automatically on first use.
- The minimum supported Rust version (MSRV) is **1.85** (see `rust-version` in
  `Cargo.toml`). CI builds and tests with it.
- Debian 13: `sudo apt install build-essential git curl`, then install `rustup`.
- Windows 10/11: the Visual Studio Build Tools ("Desktop development with C++"), then
  install `rustup`.

```text
cargo build --workspace
cargo test --workspace
```

Checks that CI enforces on both operating systems:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny --all-features check
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for conventions (English-only documentation,
Conventional Commits, branch naming) and the project constitution in
[.specify/memory/constitution.md](.specify/memory/constitution.md).

## License

GPL-3.0, see [LICENSE](LICENSE).
