# Gozen VSCode Extension (Dev)

This in-repo extension starts `gozen lsp` over stdio for `.gd` and `.gdshader` files.

## Requirements

- VS Code 1.85+
- `gozen` available on `PATH` (or set `gozen.path`)
- Node.js 18+

## Setup

```bash
cd ../vscode-extension
npm ci
npm run compile
```

## Run in Extension Development Host

1. Open `../vscode-extension` in VS Code.
2. Press `F5` to launch Extension Development Host.
3. Open a Godot project with `.gd` files.

## Settings

- `gozen.enable`: Enable/disable extension
- `gozen.path`: Path to `gozen` binary (default: `gozen`)
- `gozen.trace.server`: `off | messages | verbose`

## Notes

- This is a dev-first scaffold. Binary bundling and marketplace publishing are deferred.

## Troubleshooting

- `Gozen executable not found`: set `gozen.path` to the full binary path (for example `../target/debug/gozen`).
- PATH mismatch: VSCode may not inherit your shell PATH; prefer an absolute `gozen.path` when debugging.
- Restart the server from Command Palette: `Gozen: Restart Language Server`.
