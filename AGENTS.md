# NewTrans repository instructions

- Read `docs/development-plan.md` before changing behaviour.
- Rust stable, Edition 2024. Test first; keep platform-neutral logic in
  `crates/newtrans-core` and network code in `crates/newtrans-providers`.
- Gate: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`, `cargo test --workspace --all-features`.
- Never commit API keys, `.env`, signing material, or QuestSoft QTranslate
  binaries, text, icons or themes. The original was used only as a black-box
  feature reference (its EULA forbids decompilation).
- ahatem/QTranslate (MPL-2.0) is a design reference only; do not copy its code.
- Never log selected text, clipboard contents or translation text.
