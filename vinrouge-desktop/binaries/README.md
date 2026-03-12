# Bundled Ollama Binary

Place the Ollama binary for each target platform here before building.
Tauri will bundle whichever matches the current build target.

## Naming convention

Tauri requires the target triple as a suffix:

| Platform              | Filename                              |
|-----------------------|---------------------------------------|
| macOS Apple Silicon   | `ollama-aarch64-apple-darwin`         |
| macOS Intel           | `ollama-x86_64-apple-darwin`          |
| Linux x86_64          | `ollama-x86_64-unknown-linux-gnu`     |
| Windows x86_64        | `ollama-x86_64-pc-windows-msvc.exe`   |

## Where to get Ollama

Download from the official releases page and rename to match the convention above:
  https://github.com/ollama/ollama/releases

## Version management

To upgrade Ollama, replace the binary file and rebuild the app.
The version is entirely under your control — no automatic updates.
