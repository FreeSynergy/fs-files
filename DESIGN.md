# fs-files – Design

`fs-files` is the files program of FreeSynergy. This repo holds the **program core** (`crates/fs-files-core`, no GUI).
**Metadata** (name, media type, size, checksum, folder, trash flag) is an object in `fs-store`; the **bytes** are in a
backend behind `BlobBackend`. Other programs refer to a file by its object id and read through the `Blob` trait; they never
see a path or a backend. Contract: `files.capnp` in `fs-interfaces` v0.3.0 (draft, consumers: 0 of 2).

## State

Version 0.1.0 (stage 1: local folder). Built on fs-store 0.2.0 and fs-interfaces 0.3.0 (git tags).

| Area | State |
|---|---|
| `Files` trait (folders, get, classify, trash, search), `Transfer` (resumable upload, `Blob`), `Backends`; store-backed `FileRepo`; provider rule (ADR 0045): defaults answer `Unsupported`, `capabilities()` reports | done |
| File = AS2 `Document` / `Image` / `Video` / `Audio` by media type, `context=files`, IRI `https://<instance>/files/<id>`, derived name; folder = AS2 `Collection` | done |
| Index fields: `kind`, `name_key`, `media_type`, `folder`, `trashed`, `backend` | done |
| Trash (hide), restore, purge (bytes first, then metadata); move / copy / rename; folder move with cycle check; folder delete only when empty | done |
| Resumable upload: temp key, position = what the backend holds (survives restart, tested by reopening store and backend), wrong offset is an error, `finish` verifies size + checksum before the object exists, `abort` cleans up | done |
| Checksum `blake3:<hex>` computed while the bytes stream in; after a restart the pending bytes are streamed once through the hasher at commit | done |
| `LocalFolder` backend: key validation, no symlink followed, pending file + fsync, atomic rename, directory fsync | done (stage 1) |
| `BackendRegistry`: add (built, probed, then swapped atomically), remove (refused while files / uploads are on it, and for the active one), a file records its backend | done |
| Sensitive categories (`classify`) go to the envelope (ADR 0029); the store refuses them without encryption (`EncryptionRequired`), tested | done |
| S3 backend | stage 2, not yet |
| `sharing`, `file.subscribe`: `Unsupported` with the plain reason; versions, encryption, sync, WebDAV, previews: not built | as agreed |
| Tests | 41 |

## Structure and pattern map (ADR 0043)

| Module | Pattern | Why |
|---|---|---|
| `files.rs`, `transfer.rs`, `backends.rs` | Repository (interface) + Null Object | the public contract as traits with `Unsupported` defaults (ADR 0045); callers use `dyn Files` etc. |
| `backend/provider.rs` | Strategy (interface) + Null Object | `BlobBackend`, the provider interface: where bytes are |
| `backend/local/*` | Strategy | `LocalFolder`; `path.rs` is a Specification of the path rules, `io.rs` durable file helpers |
| `backend/key.rs` | Specification | the one place that decides what a key may be |
| `backend/digest.rs` | Strategy (hash) | running BLAKE3 per key, fallback streaming hash |
| `backend/lock.rs` | RAII / Guard | one async lock per key |
| `backend/registry.rs` | Registry with atomic snapshots | readers see the old or the new set, never a mix |
| `backend/factory.rs` | Factory + Chain of checks | config -> backend per kind; `check_backend` probes it before the swap |
| `repo/handle.rs`, `repo/provider.rs` | Repository + Facade | `FileRepo` hides store, envelope rules and backends; trait impls forward |
| `repo/backend.rs` | Adapter | `Arc<dyn Store>` as the narrow `ObjectRepository` + `ObjectQuery` |
| `repo/build.rs` | Adapter | our records to AS2 envelope + index fields |
| `repo/{folders,fileops,list,lookup}.rs` | Repository | one concern each |
| `repo/session.rs`, `repo/finish.rs` | State / Template method | upload session; the fixed finish flow |
| `repo/blob.rs` | Proxy | `FileBlob` stands in for the bytes |
| `repo/manage.rs` | Factory + Registry | `backend.add` / `backend.remove` |
| `repo/names.rs` | Specification | what a name may be, when two names are the same |
| `model/*` | Value objects | `File`, `Folder`, `Page`, `FileFilter`, ... |

## Decisions (the owner may want to change these)

* **Managed store, not real folders.** Names and folders exist in `fs-store` only. `LocalFolder` keeps bytes as plain files under
  `<root>/<xx>/<object-id>`: openable with anything, but not under the user's file names. Mirroring a real folder tree (design.md,
  open question 1) is not built; the backend interface (keys, not paths) allows a second local provider for it later.
* **Names are unique per folder** among live entries, files and folders together, case-insensitive (Unicode-normalised, via
  `fs_store::sort_key`). Creating, renaming, moving, restoring and finishing an upload into a taken name is `Conflict`. A copy
  gets `name (copy).ext`, `name (copy 2).ext`. Names never reach a path; they may not be empty, longer than 255 bytes, `.` / `..`,
  or contain `/`, `\` or control characters.
* **Folder delete:** only an empty folder. Files in the trash count as content. Moving a folder into itself or below is `Conflict`.
* **Trash:** hides from `list`, `find` and `open`; only `restore`, `purge`, `get`, `classify` and a trash search work on it. `purge` needs the
  file to be in the trash. Restore fails if the name is taken.
* **Position of an upload = bytes the backend holds.** Nothing else is stored per chunk (a store write per chunk would flood the log).
  The session object is written once at the start.
* **Backend configuration is not persisted by the core.** `FileRepo::new` takes the first backend; `add_backend` adds more at runtime.
  After a restart the program re-adds them from its own configuration under the same ids (they hold credentials; the core never
  writes them to the store or to an error).
* **One process per store:** the "name is free" checks and writes run under one lock inside the process.

## Local folder: path safety

A key is `/`-separated segments of `A-Za-z0-9._-`; a segment is never empty and never starts with `.`. That excludes `..`, absolute
paths, NUL, backslash, drive letters, spaces and Unicode. On top, every existing component of the resulting path is checked with
`symlink_metadata` and a symlink is refused (also for the `.pending` folder). Residual risk: a local attacker who can write into
the root and races between the check and the use; the root must not be writable by untrusted users.

## Crates

* `fs-store` (v0.2.0), `fs-interfaces` (v0.3.0): git tags.
* `blake3` 1.8 (CC0-1.0 OR Apache-2.0): checksums, with the `pure` feature (no C / assembly). Fast enough for a first version.
* `infer` 0.22 (MIT): media type from the first bytes when the caller gives none; pure Rust, no dependencies.
* `tokio` (fs, io, sync), `async-trait`, `thiserror`, `serde`, `serde_json`, `chrono` (MIT or Apache-2.0, already used by fs-store / fs-people).
* Dev: `tempfile`.
* Not taken: `opendal` (broad, many services we do not use; its writer hides the multipart id, so an upload could not be resumed after a restart).
