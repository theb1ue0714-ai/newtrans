# NewTrans

Select text in any application, press **Ctrl twice**, and compare
**Google · Papago · DeepL** translations in tabs — with a back-translation
check. Windows, macOS and Linux. Written in Rust (Tauri 2).

## How it works

1. Drag-select text anywhere.
2. Press **Ctrl Ctrl** (default) or **Ctrl+Q** (secondary). Both can be
   changed to a modifier double-tap or any combination of up to three keys.
3. The popup opens next to the cursor and runs a three-step chain per tab:
   - **① 원문** — language 1, detected automatically or pinned
   - **② 번역** — language 2 (Korean by default)
   - **③ 재번역** — back into language 1, to check the meaning survived
4. Switch services with the tabs, `Ctrl+Tab`, `Alt+1~3`, or by pressing the
   hotkey again while the popup is focused.

Changing language 1 or 2 in the popup pins that choice. Default languages are
Korean, English, Japanese, German, French and Spanish; more can be enabled in
settings. NewTrans starts with the OS and stays in the tray.

## Services and API keys

| Tab | Default | Key |
|---|---|---|
| Google | Free web endpoint, no key | Optional: Cloud Translation API key |
| DeepL | Needs a key | DeepL API **Free** (500,000 chars/month, key ends in `:fx`) |
| Papago | Needs a key | NAVER Cloud Platform Papago Translation (paid) |

Keys are entered in **Settings → API 키** and stored only on your computer in a
separate file readable by your account alone. NewTrans ships with no keys
and sends nothing anywhere except the translation request to the service you
use. There is no telemetry and no AI/LLM key is needed.

Read-aloud uses the voices built into your operating system.

## Platform notes

- **Windows 10/11**: Ctrl-double-tap is detected by polling key state rather
  than a low-level keyboard hook, so Windows cannot silently unhook it.
- **macOS**: allow NewTrans under *Privacy & Security → Accessibility* (and
  *Input Monitoring*) for the double-tap and for reading the selection.
- **Linux X11**: the PRIMARY selection is read directly, no key simulation.
- **Linux Wayland**: global key polling is not allowed; use the secondary
  combination where your desktop supports it.

These builds are **unsigned development builds**; Windows SmartScreen and
macOS Gatekeeper will warn on first launch.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cd apps/newtrans && npx @tauri-apps/cli@2.12.4 build   # installers
```

Live provider checks (network, keys from environment variables):

```bash
NEWTRANS_DEEPL_API_KEY=... cargo test -p newtrans-providers --test live -- --ignored
```

## License

MIT. NewTrans is an independent implementation and is not affiliated with
QuestSoft QTranslate, Google, NAVER or DeepL.
