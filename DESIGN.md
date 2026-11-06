# fs-files – Design

`fs-files` is the files program of FreeSynergy. This repo holds the **program core** (`crates/fs-files-core`, no GUI).
**Metadata** (name, media type, size, checksum, folder, trash flag) is an object in `fs-store`; the **bytes** are in a
backend behind `BlobBackend`. Other programs refer to a file by its object id and read through the `Blob` trait; they never
see a path or a backend. Contract: `files.capnp` in `fs-interfaces` v0.10.0 (draft, consumers: 0 of 2).

## State

Version 0.2.1 (stage 1: local folder, stage 2: S3, stage 3: real names). Built on fs-store 0.2.0 and fs-interfaces 0.8.0 (git tags).

| Area | State |
|---|---|
| `Files` trait (folders, get, classify, trash, search), `Transfer` (resumable upload, `Blob`), `Backends`; store-backed `FileRepo`; provider rule (ADR 0045): defaults answer `Unsupported`, `capabilities()` reports | done |
| File = AS2 `Document` / `Image` / `Video` / `Audio` by media type, `context=files`, IRI `https://<instance>/files/<id>`, derived name; folder = AS2 `Collection` | done |
| Index fields: `kind`, `name_exact`, `media_type`, `folder`, `trashed`, `backend` | done |
| Trash (hide), restore, purge (bytes first, then metadata); move / copy / rename; folder move with cycle check; folder delete only when empty | done |
| Resumable upload: temp key, position = what the backend holds (survives restart, tested by reopening store and backend), wrong offset is an error, `finish` verifies size + checksum before the object exists, `abort` cleans up | done |
| Checksum `blake3:<hex>` computed while the bytes stream in; after a restart the pending bytes are streamed once through the hasher at commit | done |
| `LocalFolder` backend: files under their **real names** (`<root>/Urlaub/Foto.jpg`), one reserved hidden folder `.fs-files/{pending,trash}`, no symlink followed, pending file + fsync, atomic no-replace rename, directory fsync | done (stage 3) |
| `BlobBackend` naming strategy: `naming()` (`ById` S3, `ByName` local) and `publish`, `relocate`, `discard`, `make_dir`, `remove_dir`, `scan`; a record says per file whether it is `named`, so id-keyed files of the first version keep working | done |
| Disk and database in step: upload, rename, move, copy, folder create / rename / move / delete, trash, restore, purge; **disk first, then the database, undone if it fails** (`repo/steps.rs`) | done |
| `rescan(folder)` (`folder.rescan`): takes over new files and folders, updates changed ones, reports vanished ones, deletes nothing; `open` refuses a file whose size no longer matches | done, provisional (see decisions) |
| Conflict rule `OnConflict` (`Refuse` default, `KeepBoth`; **no replace**) for rename, move, copy, folder rename / move and upload: `*_as` methods; `KeepBoth` gives the incoming entry `name (2).ext` (a copy `name (copy).ext`), upload also looks at the disk | done |
| Sorting for `list_sorted` / `find_sorted`: name, modified (= record changed, not disk mtime), size (folder = 0), type (ascending only); store orders by index fields `size_asc`, `size_desc`, `type_asc` | done |
| `Error::problem()`: name clash, invalid name, changed outside map to the named problem types of the contract | done |
| `BackendRegistry`: add (built, probed, then swapped atomically), remove (refused while files / uploads are on it, and for the active one), a file records its backend | done |
| Sensitive categories (`classify`) go to the envelope (ADR 0029); the store refuses them without encryption (`EncryptionRequired`), tested | done |
| `S3Backend` behind the same `BlobBackend` (feature `s3`, default on): multipart upload that survives a restart (state in the bucket), tail pieces so any chunk size works, plain put for files under one part; `S3Factory` for `backend.add`; credentials never in `Debug` or errors | done |
| One behaviour suite as functions over `&dyn BlobBackend` (`tests/common/{suite,stream}.rs`), run on `LocalFolder`, `S3Backend` over a strict fake S3, `S3Backend` over the `object_store` in-memory store, and (ignored) a live server | done |
| Live S3: official RustFS image 1.0.4 with podman, localhost only, throwaway credentials: both live tests passed; container removed | verified once (2026-11-06), not part of CI |
| `sharing`, `file.subscribe`: `Unsupported` with the plain reason; versions, encryption, sync, WebDAV, previews: not built | as agreed |
| Tests | 73 run by default (59 without the `s3` feature) + 2 ignored live tests |

## Structure and pattern map (ADR 0043)

| Module | Pattern | Why |
|---|---|---|
| `files.rs`, `transfer.rs`, `backends.rs` | Repository (interface) + Null Object | the public contract as traits with `Unsupported` defaults (ADR 0045); callers use `dyn Files` etc. |
| `backend/provider.rs` | Strategy (interface) + Null Object | `BlobBackend`, the provider interface: where bytes are |
| `backend/s3/{backend,flush}.rs` | Strategy / Template method | `S3Backend`; part, save list, then delete pieces: a fixed order that makes a crash harmless |
| `backend/s3/api.rs`, `adapter.rs` | Adapter | the few S3 calls as a narrow trait; one adapter over `object_store`, a fake in the tests |
| `backend/s3/state.rs` | Memento | multipart id, part etags and tail pieces live in the bucket and are rebuilt on load |
| `backend/s3/{layout,config,factory}.rs` | Specification / Factory | object names; JSON config -> client; `kind: s3` for `backend.add` |
| `backend/local/*` | Strategy | `LocalFolder`; `path.rs` is a Specification of the path rules, `io.rs` durable file helpers, `named.rs` the name-based operations |
| `backend/place.rs` | Value objects | `Naming` (Strategy switch), `Spot`, `Scanned` |
| `repo/steps.rs` | Command | a disk step that knows how to undo itself; `transact` = disk first, database second |
| `repo/paths.rs` | Specification | a path is derived from folder names + name, never stored for a named file |
| `repo/rescan.rs` | Visitor-like walk | disk tree and record tree walked together |
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
| `repo/names.rs` | Specification | what a name may be (two names are the same only if equal byte for byte) |
| `model/*` | Value objects | `File`, `Folder`, `Page`, `FileFilter`, ... |

## Decisions (the owner may want to change these)

* **Real names on the local disk (ADR 0055).** A file `Foto.jpg` in folder `Urlaub` is `<root>/Urlaub/Foto.jpg`; the file is
  openable with anything and the person finds it under the name they gave. S3 stays by id for now (question open in ADR 0055).
  Folders are directories; folder rename / move is one directory rename, and no file record changes (the path is derived).
* **Names per folder: as the operating system allows.** The program compares names exactly (files and folders share one
  namespace); `Foto.jpg` and `foto.jpg` may sit side by side on Linux. An exact duplicate is `Conflict`. On a case-insensitive file
  system the disk reports the clash and that is a `Conflict` too, never an overwrite. A copy gets `name (copy).ext`. A name may not
  be empty, longer than 255 bytes, `.` / `..`, contain `/`, `\` or control characters, or be `.fs-files`.
* **Never overwrite.** Every create is create-new, every move is a no-replace rename. Linux: one atomic `renameat2(RENAME_NOREPLACE)`
  (crate `rustix`, no unsafe in our code). Where the kernel or file system lacks the flag, and on other systems, the fallback is
  "look, then rename", which is **not atomic** (it could replace an *empty folder* created in between, never a file or a non-empty
  folder). Atomic: publish of an upload, file rename / move, trash, restore, folder rename / move. Not atomic: the pair of disk step
  and database step (see next point), and the fallback above. Creating a folder is `mkdir` (atomic, fails if it exists).
* **Order and crashes: disk first, then the database.** If the database step fails, the disk step is undone. If the process dies
  between the two (or the undo fails), no byte is lost: the file is where the disk says. `rescan` then takes an unknown file over as a
  new record and reports the old record as missing (kept). A trash move whose record was not written is put back by `rescan`; a
  restore whose record was not written is finished by the next `restore`. A published upload whose record was not written is a normal
  unknown file for `rescan`. Nothing runs automatically at start (the core has no start hook); the program calls `rescan`.
* **Trash and unfinished uploads** live in `<root>/.fs-files/{trash,pending}`, addressed by id or hash, never by a user name. The name
  `.fs-files` is refused for files and folders; `scan` never lists it. Restore into a taken name is `Conflict` and keeps the bytes in the trash.
* **Changes from outside the program: PROVISIONAL, the owner may change this.** `rescan(folder)` (recursive): new files and folders
  are taken over (checksum read from the bytes, type from the first bytes); a file with a different size or modification time gets new
  size and checksum; a file without bytes is **reported and its record is kept**; symlinks and odd names are listed as skipped.
  Opening a file whose size no longer matches is an error ("changed outside"), not wrong data. Nothing is detected live (no watcher),
  and the whole rescan holds the program's write lock. Same-size edits inside one mtime tick are not seen.
* **Records of the first version** (id-keyed `xx/<id>` files in the local root, no `named` flag, no `name_exact` index) keep working
  for reading; `rescan` skips them. They are not migrated: stage 1 never had a consumer. Their `xx` folders stay in the root.
* **Folder delete:** only an empty folder (on disk too: a folder with files the program does not know is `Conflict`). Files in the trash count as content. Moving a folder into itself or below is `Conflict`.
* **Trash:** moves the bytes to the trash folder and hides from `list`, `find` and `open`; only `restore`, `purge`, `get`, `classify` and a trash search work on it. `purge` needs the
  file to be in the trash. Restore fails if the name is taken.
* **Position of an upload = bytes the backend holds.** Nothing else is stored per chunk (a store write per chunk would flood the log).
  The session object is written once at the start.
* **Backend configuration is not persisted by the core.** `FileRepo::new` takes the first backend; `add_backend` adds more at runtime.
  After a restart the program re-adds them from its own configuration under the same ids (they hold credentials; the core never
  writes them to the store or to an error).
* **One process per store:** the "name is free" checks and writes run under one lock inside the process.

## S3: parts, restart, limits

S3 refuses every multipart part under 5 MiB except the last, and callers send chunks of any size. Handling: every chunk is first
written as a small **tail piece** `pending/<key>/tail/<offset>` (durable on its own, so `put` keeps its promise). When the pieces add
up to the part size (default 8 MiB, at least 5) they are read back, sent as one part, the part list is saved to `pending/<key>/state`,
and only then the pieces are deleted. Memory is one part plus one chunk, never the file. A file smaller than one part is a plain `PUT`
(no multipart, nothing to abort). The last part may be small (allowed by S3). After a restart the state is rebuilt from the bucket:
`state` gives the multipart id and parts, tail pieces below what the parts hold are leftovers of a crash and are deleted.
Limits: at most 10 000 parts, so a file can be `10 000 x part_size` (8 MiB: about 78 GiB; raise `part_size_mib` for bigger ones);
cost: each byte is written twice (tail piece, then part). Orphan multipart uploads after a crash between `create` and the first
`state` save are left to the bucket's lifecycle rule. `stat` returns the size only; the checksum comes from `commit`.
The blake3 checksum is taken while streaming; after a restart (or a failed put) the finished object is streamed once through the hasher.

## Local folder: path safety

A key is a path of real names: every segment is non-empty, at most 255 bytes, not `.` or `..`, without `\` or control characters
(NUL included), the whole at most 4096 bytes, and the first segment is not `.fs-files`. That excludes `..`, absolute paths, NUL and
the reserved folder. On top, every existing component of the resulting path is checked with `symlink_metadata` and a symlink is
refused (also inside `.fs-files`), after creating parents as well. Pending uploads are addressed by the hash of their key, trash entries by
a checked id, so no user name reaches them. Names the file system refuses (too long a path, `EINVAL`) come back as `InvalidName`
before the database changes. Residual risk: a local attacker who can write into the root and races between the check and the use
(the check is by path, not `openat2(RESOLVE_BENEATH)`); the root must not be writable by untrusted users. Hard-link and case-only
renames on case-insensitive file systems are not special-cased.

## Crates

* `fs-store` (v0.2.0), `fs-interfaces` (v0.10.0): git tags.
* `rustix` 1 (MIT OR Apache-2.0, Linux only, features `std` + `fs`): `renameat2` with `RENAME_NOREPLACE`, the atomic no-replace rename that `std` lacks. No C code with its default backend, already in the lock file through other crates.
* `blake3` 1.8 (CC0-1.0 OR Apache-2.0): checksums, with the `pure` feature (no C / assembly). Fast enough for a first version.
* `infer` 0.22 (MIT): media type from the first bytes when the caller gives none; pure Rust, no dependencies.
* `tokio` (fs, io, sync), `async-trait`, `thiserror`, `serde`, `serde_json`, `chrono` (MIT or Apache-2.0, already used by fs-store / fs-people).
* `object_store` 0.14 (MIT OR Apache-2.0, Apache Arrow project), feature `aws` only, optional (`s3`): the S3 client with a real `MultipartStore`
  trait, which exposes the multipart id and the part etags. That is what a resumable upload needs (state is persisted, so a new process
  can go on). It also gives us signing, retries and the in-memory store for tests. Its TLS comes via `reqwest` + `rustls` + `aws-lc-rs`, which
  contains C code: not pure Rust. Hence the feature: `--no-default-features` is a pure-Rust build with the local folder only. `bytes` for its payloads.
* Dev: `tempfile`.
* Not taken: `opendal` (broad, many services we do not use; its writer hides the multipart id, so an upload could not be resumed after a restart). `rusty-s3` + an own HTTP layer would be pure sans-IO but means writing the client.

## Contract

`files.capnp` in `fs-interfaces` v0.10.0 (draft 0.2). Appended for real names: `Folders.rescan @13` and `RescanReport`; `BlobBackend`
`naming`, `publish`, `relocate`, `discard`, `makeDir`, `removeDir`, `scan` (@7 to @13) with `Spot`, `Scanned`, `ScanKind`; the problem types
`nameConflictProblemType` (409), `invalidNameProblemType` (422), `changedOutsideProblemType` (409); and the statement that `Envelope.name` of a
file and a folder IS its real name (not derived). In the core `Error::problem()` makes the mapping; a name clash is still `Error::Conflict`.
`OnConflict` is a last parameter of rename, move, copy and `startUpload` (0 = refuse, so old callers keep working; the wire `copyFile` now refuses by default, the Rust `copy_file` without `_as` still keeps both). `Sort` is a last parameter of `list` and `find`. Capabilities: `file.sort`, `file.conflictRule`, `folder.rescan` (Files), `blob.rename` and `blob.scan` (backend). `moveFolder`, `pending`, `FileFilter.trashed` exist since v0.5.0. The Rust
`subscribe` placeholder still takes no listener; it comes with the feed.
