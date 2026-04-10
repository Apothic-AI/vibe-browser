# Vibe Browser

Desktop demo for the current Vibe Protocol flow:

`URL -> VIBE.md discovery or inference -> ACP agent -> generated index.html -> browser render`

## What it does

When you visit a URL, Vibe Browser:

1. Tries `/.well-known/VIBE.md`, then `/VIBE.md`
2. If neither exists, passes the target URL to the ACP agent and instructs it to infer and save a `VIBE.md`
3. Builds a render prompt that includes:
   - general Vibe Browser rendering instructions
   - an embedded markdown section with the Vibe Protocol spec
   - the full published `VIBE.md`, when one exists
   - a fallback inference request that tells the ACP agent to infer a `VIBE.md` from the live URL when one does not exist
4. Starts an ACP agent subprocess with the Rust `agent-client-protocol` crate
5. Exposes ACP file write/read capabilities inside a dedicated Vibe render cache directory
6. Instructs the agent to write a renderable `index.html`
7. Renders the produced page inside the browser UI
8. Keeps a structured ACP traffic log so you can inspect the browser-agent exchange

For now, discovered integrations such as MCP servers, Skills, A2A, OpenAPI, and other endpoints are forwarded to the agent as plain `VIBE.md` content. The browser does not interpret them specially yet.

## UI layout

- The rendered page owns the main canvas under the top bar
- The left-side rail is a real browser tab strip with add, select, and close behavior
- A right-side details drawer is collapsed by default and can be opened from the top bar for session state, discovery attempts, ACP traffic, agent notes, and the discovered `VIBE.md`
- The details drawer also tells you whether the browser used a published `VIBE.md` or an agent-inferred one
- The model selector lives inside the `ACP Agent` settings panel, not in the top bar

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
- ACP model selection from the agent’s reported `configOptions`
- `My Vibes`, a saved system-prompt-style instruction block that is injected into the ACP render prompt as `User Instructions`

The selected ACP model is persisted, so the browser reuses it across renders and app restarts when that model is still available from the agent.

`My Vibes` is also persisted. When present, the render prompt includes a dedicated `User Instructions` section that explicitly tells the ACP agent to treat those instructions as higher priority than the rest of the prompt, the embedded Vibe Protocol spec, and the discovered `VIBE.md`.

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
- `cargo test renders_discovered_vibe_document_end_to_end -- --nocapture` runs the backend end-to-end render test for `URL -> published VIBE.md -> ACP agent -> index.html`
- `cargo test infers_vibe_document_when_site_has_no_published_vibe -- --nocapture` runs the backend end-to-end render test for `URL -> inferred VIBE.md -> ACP agent -> index.html`
- `cargo test renders_discovered_vibe_document_with_opencode -- --ignored --nocapture` runs the same path against a real `opencode acp` process when OpenCode is installed and configured

## Releases

GitHub Actions builds release bundles from version tags with the workflow at `.github/workflows/release.yml`.

Release flow:

1. Update the version in `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml`.
2. Commit the version bump.
3. Create and push a tag like `v0.1.0`.
4. GitHub Actions builds and uploads the Windows NSIS installer plus Linux `deb` and `rpm` bundles to a GitHub release.
