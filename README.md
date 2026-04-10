# Vibe Browser

Desktop demo for the current Vibe Protocol flow:

`URL -> VIBE.md discovery -> ACP agent -> generated index.html -> browser render`

## What it does

When you visit a URL, Vibe Browser:

1. Tries `/.well-known/VIBE.md`, then `/VIBE.md`
2. Downloads the discovered `VIBE.md`
3. Builds a render prompt that includes:
   - general Vibe Browser rendering instructions
   - an embedded markdown section with the Vibe Protocol spec
   - the full discovered `VIBE.md`
4. Starts an ACP agent subprocess with the Rust `agent-client-protocol` crate
5. Exposes ACP file write/read capabilities inside a dedicated Vibe render cache directory
6. Instructs the agent to write a renderable `index.html`
7. Renders the produced page inside the browser UI

For now, discovered integrations such as MCP servers, Skills, A2A, OpenAPI, and other endpoints are forwarded to the agent as plain `VIBE.md` content. The browser does not interpret them specially yet.

## Current structure

Frontend:

- [src/App.tsx](/home/bitnom/Code/apothic-monorepo/apps/vibe-browser/src/App.tsx)
- [src/App.css](/home/bitnom/Code/apothic-monorepo/apps/vibe-browser/src/App.css)

Backend:

- [src-tauri/src/commands/vibe_commands.rs](/home/bitnom/Code/apothic-monorepo/apps/vibe-browser/src-tauri/src/commands/vibe_commands.rs)
- [src-tauri/src/lib.rs](/home/bitnom/Code/apothic-monorepo/apps/vibe-browser/src-tauri/src/lib.rs)

Local discovery demo:

- [public/.well-known/VIBE.md](/home/bitnom/Code/apothic-monorepo/apps/vibe-browser/public/.well-known/VIBE.md)
- [public/VIBE.md](/home/bitnom/Code/apothic-monorepo/apps/vibe-browser/public/VIBE.md)

## ACP agent settings

The browser lets you configure:

- ACP agent command
- optional ACP agent working directory

You can set any shell command in the UI that starts the ACP process Vibe Browser should use.

Examples:

```bash
opencode acp
```

```bash
uv run --project ../../yolo-python yolo-acp
```

The built-in default is:

```bash
opencode acp
```

The local `yolo-acp` command is still available as a quick-fill option in the UI when you want to test against the monorepo ACP agent instead.

## Development

```bash
pnpm install
pnpm build
pnpm tauri dev
```

On Debian 13 / Linux, Tauri also needs the WebKit and GTK development packages:

```bash
sudo apt install -y libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev
```

## Verification

- `pnpm build` should pass for the Solid frontend
- `pnpm tauri dev` should now get through the local Tauri startup path on a machine with the Debian WebKit/GTK packages installed
