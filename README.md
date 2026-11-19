# yt-client

An independent YouTube client built with **Tauri 2** (Rust backend + web frontend) and the
**YouTube Data API v3**. Playback uses the official YouTube IFrame embed (Option A) for
ToS compliance and cross-platform support.

## Stack

- Shell: Tauri 2 (Rust + webview)
- Frontend: React + TypeScript + Vite
- Backend: Rust (tokio, reqwest/rustls, rusqlite, oauth2)
- API: YouTube Data API v3
- Auth: OAuth 2.0 PKCE
- Storage: SQLite (watch cache, tokens)
- Platforms: Linux (primary), Windows fundamentals included

## Setup

1. Install Rust toolchain and Node.js.
2. Linux deps: `sudo dnf install webkit2gtk4.1-devel librsvg2-devel openssl-devel gtk3-devel libsoup3-devel javascriptcoregtk4.1-devel` (or apt equivalents).
3. Create a Google Cloud project, enable YouTube Data API v3, create an OAuth 2.0 "Desktop app" client.
4. Set env vars:
   - `YT_API_KEY` — API key for read-only calls
   - `YT_CLIENT_ID` — OAuth client id
   - `YT_CLIENT_SECRET` — OAuth client secret (optional for desktop clients)
5. `npm install`
6. `npm run tauri dev`

## Windows fundamentals

The app uses only cross-platform crates (`dirs`, `rusqlite` bundled, `reqwest` rustls) and Tauri 2,
so it builds on Windows with the standard MSVC toolchain. CI builds both Linux and Windows (see `.github/workflows`).

## License

MIT