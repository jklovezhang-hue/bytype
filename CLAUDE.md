# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

**ByType** — a Windows press-to-talk voice input tool (Typeless-style). Hold a hotkey, speak, and the
text is injected into whatever app currently has focus. Pipeline: global keyboard hook → mic capture →
local **SenseVoice** ASR (CPU, via `sherpa-rs`) → **LLM** cleanup/translate/command (OpenAI-compatible
relay) → clipboard paste. Ships both as a headless CLI bin and a **Tauri 2** desktop app (tray + recording
overlay). Windows-only.

## Build environment (read first — non-obvious, easy to trip on)

- **`cargo`/`rustc` are NOT on PATH** in fresh shells, and `sherpa-rs` runs `bindgen` which needs **libclang**.
  Prefix every cargo command (PowerShell):
  ```powershell
  $env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;$env:Path"; $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
  ```
- Use **PowerShell** (Windows toolchain: MSVC, cpal, the `windows` crate). The unix-bash environment lacks the toolchain.
- The ASR model lives at `models/sensevoice/{model.onnx, tokens.txt}` (int8). `models/`, `target/`, `dist/`,
  `node_modules/`, `src-tauri/gen/`, and `config.toml` are gitignored.
- **`config.toml` is gitignored and contains a real relay API key** — never commit it. `config.example.toml`
  is the committed template; the app needs a `config.toml` to run.

## Common commands

```powershell
# Core unit tests (the only meaningful test target; src-tauri/bin have ~no tests)
cargo test -p voice-input
cargo test -p voice-input hotkey::state            # one module
cargo test -p voice-input esc_passes_through_when_idle   # one test by name

cargo build                                         # whole workspace (core + src-tauri)

# Headless CLI (needs config.toml + models at repo root)
cargo run                                           # the `voice-input` bin

# GUI dev (from repo root; Node is on PATH). tauri dev sets the exe CWD to src-tauri/,
# which is why config/model lookup uses Config::load_resolved() (see below).
npm run tauri dev
npm run dev                                          # frontend only (Vite, port 1420)
npm run build                                         # bundle both HTML entries to dist/
npm run tauri build                                  # produce the installable app

# Offline verification of sub-pipelines (no hotkey/mic needed)
cargo run --example transcribe_wav -- some.wav
cargo run --example correct_text -- "嗯那个文本" [clean|polish|summary|translate]
cargo run --example command_text
```

## Architecture (the big picture)

Cargo **workspace**: the root crate is the core library `voice_input` (package name `voice-input`, also a
headless bin); member `src-tauri` is the GUI app (package `bytype`, lib `bytype_lib`) depending on the core
crate. Frontend lives in `src-ui/` (React + TS + Tailwind + Vite), built to `dist/`.

**`src/engine.rs` is the heart.** `run_with(config, observer)` orchestrates everything; `run(config)` is a
thin wrapper passing a `NoopObserver`. Flow:

1. Spawns `hotkey::run` on a thread → installs a `WH_KEYBOARD_LL` low-level hook (`src/hotkey/mod.rs`).
2. The hook normalizes raw key events into `state::Event`, feeds them to the **pure state machine**
   `HotkeyState` (`src/hotkey/state.rs`) which returns `Decision { action, suppress }`, maps `Action` →
   `HotkeyAction`, and sends it over a **crossbeam channel**.
3. The engine loop receives `HotkeyAction`: `StartRecording` → `audio::Recorder::start`; `Stop*` →
   `Recorder::stop` → `asr::Transcriber::transcribe` → `corrector` (correct / translate / command) →
   `inject::inject_text` (set clipboard + synthesize Ctrl+V via `SendInput`). `Cancel/Discard` drops the recorder.
4. At each transition the engine calls `observer.on_state(OverlayState::{Recording|Processing|Done|Cancelled|Failed})`.
   The **`EngineObserver` trait is the GUI-decoupling seam**: the CLI uses `NoopObserver`; the Tauri app uses
   `TauriObserver` (in `src-tauri/src/lib.rs`) which positions+shows the overlay window and emits a `bt:state`
   event to the webview.

**Hotkeys** (press-to-talk, hold ≥ `MIN_HOLD_MS` = 300ms; shorter = discard). Configurable in `[hotkey]`,
defaults: `LWin` = transcribe+clean, `LWin+LAlt` = translate→English, `LWin+LCtrl` = run spoken command on
the currently-selected text. Priority: command > translate > transcribe.

**Cancel mid-recording (skips the LLM):** press **Esc** (hook → state machine → `CancelRecording`, Esc is
swallowed) or **click the overlay pill** (webview `invoke("cancel_recording")` → `ControlHandle.cancel()`
injects `CancelRecording` into the same channel). Both converge on the engine dropping the recorder.

**`src/corrector.rs`** calls an OpenAI-compatible `/v1/chat/completions` relay (blocking `reqwest`). Behavior
is config-driven: `[llm] mode` presets (clean/polish/summary), `system_prompt`/`translate_prompt`/`command_prompt`
overrides, a `vocabulary` list, per-app style (`[[app_style]]` matched by foreground process name via
`src/foreground.rs`), and fallbacks (disabled / timeout / too-short → return raw text).

**`src/config.rs`** — all fields have defaults (`#[serde(default)]` + `Default` impls), so partial configs
never error. `Config::load_resolved()` finds `config.toml` independent of CWD (searches CWD → exe dir →
parent dirs) and resolves a relative `asr.model_dir` to an absolute path — this exists because `tauri dev`
runs the exe with CWD = `src-tauri/`, not the repo root.

**Tauri layer** (`src-tauri/src/lib.rs`): tray menu (设置/退出), single-instance, close-hides-to-tray, plus
the overlay window. Permissions are in `src-tauri/capabilities/*.json`.

## Critical invariants (violating these breaks the app subtly)

- **Self-injected input must be tagged.** `INJECTED_TAG` (`0x564F_4943`, in `src/lib.rs`) is written to
  `dwExtraInfo` on every `SendInput` we emit; the hook ignores tagged events so our Ctrl+V / Win-release
  synthesis doesn't feed back into itself.
- **Win-key "disguise release."** Suppressing a *lone* Win keyup leaves the OS thinking Win is held → Start
  menu pops and subsequent keys become `Win+key`, and our Ctrl+V becomes `Win+Ctrl+V` (paste fails). Fix
  (in the hook): when a hold ends with only swallowed keys, suppress the physical Win-up and inject
  `[Ctrl tap, Win up]` (tagged). The state machine encodes this: `passthrough_seen` tracks whether the OS saw
  any non-swallowed key; on `PrimaryUp` of a combo, `suppress = !passthrough_seen`.
- **The overlay window must never steal focus.** The engine pastes into whatever app has focus; if showing the
  overlay activated it, paste would land nowhere. Enforced by `focus: false` in `tauri.conf.json` **and**
  `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` applied to the overlay HWND at setup. Do not call `set_focus` on the
  overlay.
- The frontend has **two Vite entries**: `index.html` → `src-ui/main.tsx` (main/settings window, label `main`)
  and `overlay.html` → `src-ui/overlay.tsx` (recording pill, label `overlay`). Keep `vite.config.ts`
  `rollupOptions.input` in sync with `tauri.conf.json` window URLs.

## Development workflow

This project is built in phases using the **superpowers** skill chain: brainstorming → writing-plans →
subagent-driven-development → finishing-a-development-branch. Design specs and step-by-step implementation
plans live in `docs/superpowers/specs/` and `docs/superpowers/plans/`. GUI work is decomposed G1–G6
(G1 Tauri shell, G2 recording overlay, G3 beeps, G4 settings UI, G5 first-run/model-download wizard,
G6 installer + about page). OS-integration code (hook, audio, injection, Tauri windows) is verified by
live testing on a real machine, not unit tests; pure logic (state machine, config, prompts, geometry) is TDD'd.
