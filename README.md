# softilium

A high-precision, humanized Monkeytype client built in Rust. Interacts directly with Chromium instances using the Chrome DevTools Protocol (CDP) via asynchronous WebSockets.

## Overview

Unlike standard keystroke injection utilities or DOM automation wrappers that degrade under high speeds, `softilium` connects directly to the browser's CDP WebSocket interface (`/json` & `Input.dispatchKeyEvent`). It models real human typing biometrics, ensuring tests pass anti-cheat consistency and telemetry analysis.

## Key Features

- **Direct CDP Transport**: Communicates over asynchronous WebSockets (`tokio-tungstenite`) with sub-millisecond dispatch latency.
- **Dynamic DOM Active-Word Tracking**: Evaluates strictly `#words .word.active` and untyped letters (`letter:not(.correct):not(.incorrect)`), completely avoiding DOM line-shift bugs during multi-line time tests.
- **Biometric Pacing & Velocity Waves**: Implements harmonic velocity curves, intra-word burst acceleration, and cognitive pauses on word transitions.
- **Physical Key Dwell Times**: Simulates hardware switch actuation durations (38–68 ms) rather than instantaneous synthetic releases.
- **Digraph & N-gram Modeling**: Accelerates on high-frequency letter pairs (`th`, `er`, `in`, `ст`, `но`, etc.) matching human muscle-memory patterns.
- **Natural Typo Simulation (`Legit Pro`)**: Models occasional misstrikes on adjacent keyboard physical layouts (QWERTY and JCUKEN) followed by visual reaction delays and Backspace corrections (~97.5–99% accuracy).
- **Stealth Video Triggers**:
  - `Hotkey (F8)`: Listens for in-browser F8 actuation so terminal windows remain hidden during screen capture or streaming.
  - `Manual First-Key`: Awaits the user's manual first keystroke on their physical keyboard before seamlessly continuing automation.
  - `Delay`: Provides an 8-second countdown to switch applications and configure recording software.

## Architecture

```
softilium/
├── Cargo.toml
├── src/
│   ├── browser.rs    # Cross-platform browser discovery and CDP handshake
│   ├── cdp.rs        # Asynchronous CDP WebSocket client and key dispatching
│   ├── cli.rs        # CLI argument parsing and interactive terminal UI
│   ├── engine.rs     # Velocity curves, timing profile, and n-gram modeling
│   └── main.rs       # Execution loop and test completion telemetry
└── tests/
```

## Installation & Build

Ensure Rust (1.75+) is installed:

```bash
cargo build --release
```

The compiled binary will be located at `target/release/softilium`.

## Usage

### Interactive Mode

Simply run the binary without flags to enter the interactive prompt:

```bash
cargo run --release
```

### CLI Mode

```bash
# Run at 200 WPM with humanized velocity waves and realistic typos
softilium --wpm 200 --mode legit

# Run at 250 WPM with 100% accuracy and stealth F8 hotkey trigger
softilium --wpm 250 --mode smooth --trigger hotkey

# Connect to a custom debugging port
softilium --wpm 180 --cdp 127.0.0.1:9222
```

### Supported Browsers

Automatically detects and configures:
- Helium
- Google Chrome
- Chromium
- Brave
- Microsoft Edge
- Yandex Browser

## License

MIT License. See [LICENSE](LICENSE) for details.
