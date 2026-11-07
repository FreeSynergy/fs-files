# Handoff (programming)

* 2026-11-07, Thread 3 point 1: `fs-files-server` (contract `files.capnp` over a Unix socket), `FileFeed` (Observer) in the core, `File` / `Folder` carry their envelope. Branch `server`, not tagged. State and open points: `DESIGN.md`, section "Server".
* 2026-11-07, Thread 4 point 4: `fs-files-client` (`RpcFiles`: `Files` and `Transfer` over the socket) and `fs-files-app` on it (`--socket`, no own store or backend; `--root` / `--store` refused). Branch `gui`, not tagged, not reviewed. App is pinned to `fs-components` `8c38a3b` (main), not to the unpushed `files-gui` commits. State, limits and `Unsupported`: `DESIGN.md`, sections "Client" and "Frontend".
