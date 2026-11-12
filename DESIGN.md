# fs-files – Design

`fs-files` is the files program of FreeSynergy. This repo holds the **program core** (`crates/fs-files-core`, no GUI) and the **server** (`crates/fs-files-server`, the contract over a Unix socket).
**Metadata** (name, media type, size, checksum, folder, trash flag) is an object in `fs-objects`; the **bytes** are in a
backend behind `BlobBackend`. Other programs refer to a file by its object id and read through the `Blob` trait; they never
see a path or a backend. Contract: `files.capnp` in `fs-interfaces` (tag v0.0.18, with `Files.types`) (draft, consumers: 0 of 2).

## State

Version 0.0.9 (long calls of the file window run on a worker; fs-components v0.0.12); 0.0.8 was (the file window is the standard window; fs-components v0.0.11, fs-interfaces v0.0.19; folders first; types by bytes, ending and text; see "Lift to fs-components v0.0.11"); 0.0.7 was: timeouts: no call to the server waits for ever; 0.0.6 was the Q0 migration: one Blinc place, version handshake, `--facet`, typing test; 0.0.5 was tagged but its release was red, left as it is; 0.0.4 was tagged but its release failed: `--version` was missing; window rebuilt on the generated Admin frontend; earlier: 0.6.0 tagged v0.6.0, header corrected 2026-11-08; 0.5.0 was: stage 1: local folder, stage 2: S3, stage 3: real names, stage 4: the server, 0.5.0: store identity, typed problem extensions, uploads paged in the store). Built on fs-objects v0.0.1, fs-interfaces v0.0.18 and fs-components v0.0.8 (git tags). Interface status: draft, provisional, consumers 0 of 2.

| Area | State |
|---|---|
| `Files` trait (folders, get, classify, trash, search), `Transfer` (resumable upload, `Blob`), `Backends`; store-backed `FileRepo`; provider rule (ADR 0045): defaults answer `Unsupported`, `capabilities()` reports | done |
| File = AS2 `Document` / `Image` / `Video` / `Audio` by media type, `context=files`, IRI `https://<instance>/files/<id>`, the envelope name IS the real name; folder = AS2 `Collection` | done |
| Index fields: `kind`, `name_exact`, `media_type`, `folder`, `trashed`, `backend` | done |
| Trash (named files: bytes move to the trash folder, after a check that they are the recorded ones), restore, purge (bytes first, then metadata); move / copy / rename; folder move with cycle check; folder delete only when empty | done |
| Resumable upload: temp key, position = what the backend holds (survives restart, tested by reopening store and backend), wrong offset is an error, `finish` compares what the backend held with what it stored (two readings of the same bytes: it catches a short or doubled write, not a bad disk) and takes the checksum from the stream, before the object exists; the bytes stay until the record is written, and a retry goes on, `abort` cleans up | done |
| Checksum `blake3:<hex>` computed while the bytes stream in; after a restart the pending bytes are streamed once through the hasher at commit | done |
| `LocalFolder` backend: files under their **real names** (`<root>/Urlaub/Foto.jpg`), one reserved hidden folder `.fs-files/{pending,trash}`, no symlink followed, pending file + fsync, atomic no-replace rename, directory fsync | done (stage 3) |
| `BlobBackend` naming strategy: `naming()` (`ById` S3, `ByName` local) and `publish`, `relocate`, `discard`, `make_dir`, `remove_dir`, `scan`; a record says per file whether it is `named`, so id-keyed files of the first version keep working | done |
| Disk and database in step: upload, rename, move, copy, folder create / rename / move / delete, trash, restore, purge; **disk first, then the database, undone if it fails** (`repo/steps.rs`) | done |
| `rescan(folder)` (`folder.rescan`): takes over new files and folders, updates changed ones, reports vanished ones, deletes nothing; `open` refuses a file whose size no longer matches | done, provisional (see decisions) |
| Conflict rule `OnConflict` (`Refuse` default, `KeepBoth`; **no replace**) for rename, move, copy, folder rename / move and upload: `*_as` methods; `KeepBoth` gives the incoming entry `name (2).ext` (a copy `name (copy).ext`), upload also looks at the disk | done |
| Sorting for `list_sorted` / `find_sorted`: name, modified (= record changed, not disk mtime), size (folder = 0), type (ascending only); store orders by index fields `size_asc`, `size_desc`, `type_asc` | done |
| Filters by size (`min_size`, `max_size`, a folder counts as 0) and date (`modified_*`, `created_*`; after inclusive, before exclusive) in `FileFilter`; no tags | done |
| `empty_trash` (purges all, reports `failed`), `restore_to(id, folder, on)` (another folder; unset = top level), `restore_as(id, on)` (keep both instead of failing on a taken name) | done |
| `get_folder` (`folder.resolve`, `Folders.getFolder @16`): resolve a folder by id | done |
| `pending_uploads` (`Transfer.pendingUploads @3`, `UploadInfo`): unfinished uploads oldest first, with `received`; survives a restart | done |
| `SupportReport.instance` = `Files::identity()` = the id of the store (`StoreInfo`, set by `FileRepo::new`, `with_info` for the narrow constructor); empty for a provider without a store; never in the log | done, tested over the socket (same as `store.identity()`, same after a restart, other for a new store, empty for a stub) |
| Typed problem extensions (`id`, `kind`, `issue`, `name`, `expected`, `got`; table below): the core error carries a `Note` (text + facts), `Error::facts()` gives them, the server states them next to the unchanged detail | done, one socket test per problem type that can be provoked |
| Pages: `Folders.list`, `Files.list`/`query`, `Search.find` cut in the store (`limit` + offset, rows read stay linear); `pendingUploads` cut in the store since 0.5.0 (`Transfer::pending_uploads_page`); `BlobBackend.scan` still cuts the listing of the backend (see open points) | done, measured (`tests/counting.rs`) |
| `Error::problem()`: name clash, invalid name, changed outside map to the named problem types of the contract | done |
| `BackendRegistry`: add (built, probed, then swapped atomically), remove (refused while files / uploads are on it, and for the active one), a file records its backend | done |
| Sensitive categories (`classify`) go to the envelope (ADR 0029); the store refuses them without encryption (`EncryptionRequired`), tested | done |
| `S3Backend` behind the same `BlobBackend` (feature `s3`, default on): multipart upload that survives a restart (state in the bucket), tail pieces so any chunk size works, plain put for files under one part; `S3Factory` for `backend.add`; credentials never in `Debug` or errors | done |
| One behaviour suite as functions over `&dyn BlobBackend` (`tests/common/{suite,stream}.rs`), run on `LocalFolder`, `S3Backend` over a strict fake S3, `S3Backend` over the `object_store` in-memory store, and (ignored) a live server | done |
| Live S3: official RustFS image 1.0.4 with podman, localhost only, throwaway credentials: both live tests passed; container removed | verified once (2026-11-06), not part of CI |
| `FileFeed` (Observer, `file.subscribe`): `Created` / `Updated` / `Deleted` of files and folders; one Decorator over the object repository tells every write, upload sessions never | done |
| `File` and `Folder` carry the envelope as stored; every write answers the object as stored (the store sets the times), so an answer equals what `get` says afterwards | done |
| Server `fs-files-server`: every method of `Files`, `Folders`, `Transfer`, `Upload`, `Blob` (common), `Search`, `Backends` and `BlobBackend` (provider side), `subscribe` with `ended`, `supportReport`; errors as typed problems; binary with `--socket --data --actor --instance` and `FS_FILES_ROOT` | done, draft |
| `sharing` (empty placeholder interface): `Unsupported` (`share.create`); versions, encryption, sync, WebDAV, previews: not built | as agreed |
| Tests | 255 run by default at 0.0.9 (254 at 0.0.8; all crates; at 0.0.7: 213, core 142, server 71); 2 ignored live tests. The server tests run over a real socket against a real `FileRepo`, compare values, and run the real binary across a restart |

## Structure and pattern map (ADR 0043)

| Module | Pattern | Why |
|---|---|---|
| `files.rs`, `transfer.rs`, `backends.rs` | Repository (interface) + Null Object | the public contract as traits with `Unsupported` defaults (ADR 0045); callers use `dyn Files` etc. |
| `backend/provider.rs` | Strategy (interface) + Null Object | `BlobBackend`, the provider interface: where bytes are |
| `backend/s3/{backend,flush}.rs` | Strategy / Template method | `S3Backend`; part, save list, then delete pieces: a fixed order that makes a crash harmless |
| `backend/s3/api.rs`, `adapter.rs` | Adapter | the few S3 calls as a narrow trait; one adapter over `object_store`, a fake in the tests |
| `backend/s3/state.rs` | Memento | multipart id, part etags and tail pieces live in the bucket and are rebuilt on load |
| `backend/s3/{layout,config,factory}.rs` | Specification / Factory | object names; JSON config -> client; `kind: s3` for `backend.add` |
| `backend/local/*` | Strategy | `LocalFolder`; `path.rs` is a Specification of the path rules, `io.rs` durable file helpers, `named.rs` the name-based operations (no pattern of its own: small steps) |
| `backend/place.rs` | Value objects | `Naming` (Strategy switch), `Spot`, `Scanned` |
| `repo/steps.rs` | Command | a disk step that knows how to undo itself; `transact` = disk first, database second |
| `repo/paths.rs` | Specification | a path is derived from folder names + name, never stored for a named file |
| `repo/rescan.rs` | Visitor | disk tree and record tree walked together |
| `backend/nested.rs` | Specification | is there a backend (`.fs-files`) below this folder; bounded, fail closed |
| `backend/key.rs` | Specification | the one place that decides what a key may be |
| `backend/digest.rs` | Strategy (hash) | running BLAKE3 per key, fallback streaming hash |
| `backend/lock.rs` | RAII / Guard | one async lock per key |
| `backend/registry.rs` | Registry with atomic snapshots | readers see the old or the new set, never a mix |
| `backend/factory.rs` | Factory | config -> backend per kind; `check_backend` probes it before the swap |
| `repo/handle.rs`, `repo/provider.rs` | Repository + Facade | `FileRepo` hides store, envelope rules and backends; trait impls forward |
| `repo/backend.rs` | Adapter | `Arc<dyn Store>` as the narrow `ObjectRepository` + `ObjectQuery` |
| `repo/build.rs` | Adapter | our records to AS2 envelope + index fields |
| `repo/{folders,fileops,list,lookup}.rs` | Repository | one concern each |
| `repo/session.rs`, `repo/finish.rs` | State / Template method | upload session; the fixed finish flow |
| `repo/pending.rs` | Iterator over the Repository | a page of unfinished uploads after a place, asked of the store with a limit; stale sessions removed once at the start |
| `note.rs` | Value object | `Note` (message + facts), `Kind`, `Subject` |
| `facts.rs` | none | `Error::facts()`: what an error knows, typed |
| `fs-files-server/facts.rs` | none | facts as problem extensions |
| `repo/blob.rs` | Adapter | `FileBlob` presents the bytes under a key of a backend as the `Blob` trait |
| `repo/manage.rs` | Factory + Registry | `backend.add` / `backend.remove` |
| `repo/names.rs` | Specification | what a name may be (two names are the same only if equal byte for byte) |
| `model/*` | Value objects | `File`, `Folder`, `Page`, `FileFilter`, ... |
| `feed.rs` | Observer | `FileFeed`, `FileListener`, `Registry` (subject), `Subscription` (drop = unsubscribe); the trait first |
| `repo/notify.rs`, `repo/subscribe.rs` | Decorator, Observer (subject) | `Notifying` wraps the `ObjectRepository` and tells each write, so no operation can forget it |
| `fs-files-server/{files,folders,transfer,search,backends}_service.rs` | Adapter + Factory / Facade | wire in, one trait call, wire out; the root hands out the capabilities; depends on the traits only |
| `fs-files-server/{folder,file,trash}_ops.rs` | Adapter | the methods of `Folders` by concern, behind the one-line trait impl in `folders_service.rs` |
| `fs-files-server/{upload,blob}_service.rs` | Adapter + RAII / Guard | `Upload` (calls in arrival order, writes run to their end) and `Blob` (clamped ranges) in front of the core's objects |
| `fs-files-server/backend_service.rs`, `backend_wire.rs` | Adapter | the provider interface `BlobBackend` in front of a core `BlobBackend`; for a backend that runs as a program |
| `fs-files-server/feed.rs` | Observer + Adapter | `FileFeed` into the shared forwarder; the envelope is read when the change is told |
| `backend/confined.rs` | Factory + Specification | `ConfinedLocalFactory`: a `local` backend only below a base; `FileRepo::with_local_base` |
| `fs-files-server/places.rs` | Specification | the folder of the files must not hold the data directory (`check_places`) |
| `fs-files-server/errors.rs` | Adapter | core errors as RFC 9457 problems; `finish` = `run_to_end` |
| `fs-files-server/wire/*` | Adapter, Specification | ids, texts with limits, cursor, entries and pages, filter, reports |
| `fs-files-types/{file,folder,storage,album,picture}.rs` | Registry entry (`DescribesType`, trait first) + Builder | each data type reports itself to the generated frontend: fields, all commands (ADR 0066) |
| `fs-files-types/{file_later,picture_later}.rs` | Builder | the commands of `requirements.md` the core does not have, described anyway |
| `fs-files-types/support.rs`, `requirements.rs` | Specification | which command the core answers (the one table, until `types.capnp` has `supported`); which capability of the requirements lives in which command |
| `fs-files-types/wire.rs` | Adapter | plain descriptions of `fs-types` v0.0.2 to and from the generated `types_capnp` of the contract tag |
| `fs-files-server/types_service.rs` | Adapter | `Files.types`: the registry as the `TypeCatalog` capability |
| `fs-files-app/remote.rs` | Proxy | a worker thread owns the Cap'n Proto connection |
| `fs-files-app/backend_{file,folder,storage,picture}.rs` | Adapter + command dispatch by name | `files.capnp` behind the `Backend` trait of `fs-admin-frontend`, one per data type |
| `fs-files-app/listing.rs` | Iterator over pages | the list is `find` with an empty search word, read page by page (strategy rule) |
| `fs-files-app/transfer.rs` | Template method | upload and download in chunks between a `file:` link and a file of the program |
| `fs-files-app/{values,catalog,words}.rs`, `window.rs`, `gui.rs` | Adapter / Strategy (`Say`) / Facade | records and typed values; the catalog read from the server; the English words; the window shell; the start |

## Decisions (the owner may want to change these)

* **Real names on the local disk (ADR 0055).** A file `Foto.jpg` in folder `Urlaub` is `<root>/Urlaub/Foto.jpg`; the file is
  openable with anything and the person finds it under the name they gave. S3 stays by id for now (question open in ADR 0055).
  Folders are directories; folder rename / move is one directory rename, and no file record changes (the path is derived).
* **Names per folder: as the operating system allows.** The program compares names exactly (files and folders share one
  namespace); `Foto.jpg` and `foto.jpg` may sit side by side on Linux. An exact duplicate is `Conflict`. On a case-insensitive file
  system the disk reports the clash and that is a `Conflict` too, never an overwrite. A copy gets `name (copy).ext`. A name may not
  be empty, longer than 255 bytes, `.` / `..`, contain `/`, `\` or control characters, or be `.fs-files`.
* **Never overwrite.** Every create is create-new, every move is a no-replace rename. Linux: one atomic `renameat2(RENAME_NOREPLACE)`
  (crate `rustix`, no unsafe in our code). Without the flag (old kernel, a file system that lacks it, other systems) a **file** is moved by hard link + unlink (never replaces; both names exist for an instant, and after a crash
  in that instant two names stay, which `rescan` shows as a new file; tested by switching the native call off with `LocalFolder::without_native_rename`); a **folder** by "look, then rename", which is not atomic and could replace an *empty folder* created in between (never a
  file, never a folder with content). `LocalFolder::commit` never replaces either. Atomic: publish of an upload, file rename / move, trash, restore, folder rename / move. Not atomic: the pair of disk step
  and database step (see next point), and the fallback above. Creating a folder is `mkdir` (atomic, fails if it exists).
* **Order and crashes: disk first, then the database.** If the database step fails, the disk step is undone. If the process dies
  between the two (or the undo fails), no byte is lost: the file is where the disk says. `rescan` then takes an unknown file over as a
  new record and reports the old record as missing (kept). A trash move whose record was not written is put back by `rescan`; a
  restore whose record was not written is finished by the next `restore` (only if the bytes at the place are the recorded ones: the checksum decides). A published upload whose record was not written is a normal
  unknown file for `rescan`. Nothing runs automatically at start (the core has no start hook); the program calls `rescan`.
* **Trash and unfinished uploads** live in `<root>/.fs-files/{trash,pending}`, addressed by id or hash, never by a user name. The name
  `.fs-files` is refused for files and folders; `scan` never lists it. Restore into a taken name is `Conflict` and keeps the bytes in the trash.
* **Changes from outside the program: PROVISIONAL, the owner may change this.** `rescan(folder)` (recursive): new files and folders
  are taken over (checksum read from the bytes, type from the first bytes); a file with a different size or modification time gets new
  size and checksum; a file without bytes is **reported and its record is kept**; symlinks and odd names are listed as skipped.
  Before a destructive step (trash, rename, move, restore) the checksum is always compared for files up to 256 MiB (so a stranger's file with a copied modification time does not pass); above that size the modification time is trusted. Opening a file checks only size and modification time. Opening a file whose size or modification time no longer matches the record is an error ("changed outside"), not wrong data; before a file is moved, trashed or restored by a record, its bytes are checked (size, modification time, on doubt the checksum) and a mismatch changes nothing. A folder rename or move never merges into a path that exists on the disk (`Conflict`), and is refused up front (`InvalidName`) when its deepest subfolder would pass 4095 bytes or 512 levels. Rescan reads all records once, walks with a work list (no recursion, depth limit 512), reports what it cannot handle and goes on, never rebinds a record to a stranger's file after a crash in the trash, and removes pending blobs of no upload only when they are older than 30 days (`FileRepo::with_orphan_after`, floor one hour). The store's filtered queries cost grows with every index condition (about a second per condition per 1000 records in a debug build); `list` and `require_free` therefore ask with one condition each (see `live_in` / `live_place` below). With 2000 records in a release build an upload takes about 5 ms and a rename about 1.5 ms (measured before `live_place`; the debug-build times of lists of 5000 files in one folder and of 1000 folders with the same file name are not re-measured here). The trash folder is read (`BlobBackend::inspect`, size and checksum) before `restore`, `purge` and the rescan touch an entry: a stray file there is never moved or deleted for a record, and the rescan keeps a live record whose trash spot holds a stray. Records are read in one query, not in pages (a delete between two pages cannot skip or repeat a record); the session records are deleted under the global lock. `pending_uploads` is one query too. `live_in` and `live_place` (folder and name) are index fields that exist for live entries only, so `list` and `require_free` each need one condition (records of earlier versions lack them). `FileRepo::with_orphan_after` has a floor of one hour; stores that share a root must share the limit, an upload older than it is removed by whichever store rescans. On a local backend the upload key is not probed (a person's file may have that name). Not done: the folder-fit check counts folders only (a long file name in the deepest folder can still pass the limit; the disk then answers `InvalidName` at that file). Nothing is detected live (no watcher),
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

## S3: commit crash safety

One finish per upload at a time (a per-upload lock); a second finish returns the first's file, and `abort` takes the same locks. The marker below is saved before anything else in `commit`, and `put` refuses once it is set, so a cut-short finish can only be finished. After a commit whose record could not be written, `received` reports the committed size and a `write` at 0 is refused.

`commit` saves a completion marker (with the object's size) in the state before it makes the object, and first removes an older
object of the same key, so from the marker on an object of that size can only be this upload's. A retry after a crash at any step
sees the marker and the object and only cleans up; before the object exists it makes it again (the pieces are not deleted until the
object is there). Dropping a pending upload (`discard(Pending)`) removes parts, pieces and state but **never the finished object**; it
aborts the multipart upload only if the object does not exist yet. Tested on the fake S3 with a crash at every call of `commit`, for
the multipart and the plain-put path, then retrying and then dropping. Residual: a dropped upload whose object was already made
leaves that object in the bucket without a record (safe, but garbage). A real S3 may answer a second `complete` differently from the
fake; the marker avoids the call.

## Local folder: path safety

A key is a path of real names: every segment is non-empty, at most 255 bytes, not `.` or `..`, without `\` or control characters
(NUL included), the whole at most 4096 bytes, and the first segment is not `.fs-files`. That excludes `..`, absolute paths, NUL and
the reserved folder. On top, every existing component of the resulting path is checked with `symlink_metadata` and a symlink is
refused (also inside `.fs-files`), after creating parents as well. Pending uploads are addressed by the hash of their key, trash entries by
a checked id, so no user name reaches them. Names the file system refuses (too long a path, `EINVAL`) come back as `InvalidName`
before the database changes. Residual risk: a local attacker who can write into the root and races between the check and the use
(the check is by path, not `openat2(RESOLVE_BENEATH)`); the root must not be writable by untrusted users. Hard-link and case-only
renames on case-insensitive file systems are not special-cased.

## Server (`crates/fs-files-server`)

The contract `files.capnp` over a Unix socket (`fs_interfaces::rpc`), in front of `Files`, `Transfer`, `Backends` and `FileFeed`
(`FilesServer::new(files, transfer, backends, feed)`; a program passes its one `FileRepo` for all four). It translates and nothing
else. Binary: `fs-files-server [--socket] [--data] [--actor] [--instance]`; the folder the files live in is `FS_FILES_ROOT` (absolute;
default `<data>/files`), kept under real names. The store is `<data>/objects.db`. Nothing runs at start (no rescan); a program calls `rescan`.

**Everything the contract names is built or answers `Unsupported`.** Built: all 7 methods of `Files`, all 17 of `Folders`, `Transfer`
(4), `Upload` (4), `Blob` (2), `Search` (1), `Backends` (3), `BlobBackend` (17, as `BlobBackendServer`, not handed out by the `Files` root: a
backend that runs as its own program serves it). `Unsupported` (501, `share.create` in `detail`): `Files.sharing`, because `Sharing` has no
methods yet, and `file.sort` descending by media type (the contract says so). A provider that lacks `file.subscribe` answers `subscribe` with its own
reason; a backend that cannot do a method answers its own `Unsupported`.

**Problem types.** All come from the contract since v0.35.0 (constants in `files_capnp`; the core maps the three name problems with `Error::problem`): `files/not-found` 404, `files/conflict` 409, `files/wrong-offset` 409, `files/invalid` 422 (bad backend
configuration), `files/encryption-required` 409 (ADR 0029: the store has no key), `files/stored-data-invalid` 500, `files/backend-failed`
500; and from `fs_interfaces::rpc`: `invalid-request` 422 (a text or id that cannot be right), `too-large` 413, `feed-ended` 410.

**Problem extensions (since fs-interfaces v0.39; the detail text is unchanged).** Extensions are statements of the server; the keys `id`, `kind`, `issue` are the standard ones, `name`, `expected`, `got` belong to the files problem types. `kind` is `file`, `folder`, `upload` or `backend` (`id` of a backend is its name, not a uuid).

| Problem type | Extensions |
|---|---|
| `not-found` | `id`, `kind` of what is missing (file, folder, upload, backend); a bare store miss gives `id` only |
| `name-conflict` | `name` (the name that is taken) |
| `invalid-name` | `name`, `issue` (the reason, one entry: `empty or longer than 255 bytes`, `reserved`, `refused-by-disk`, `too-long`, `path-too-long`, `nested-too-deep`, ...); a key error gives `issue` only |
| `changed-outside` | `id`, `kind=file` |
| `wrong-offset` | `expected` (what the backend holds, where the next chunk must start), `got` (what the caller sent), decimal |
| `conflict` | `id`, `kind` where the call clashes with one object (a file not in the trash, a folder not empty, a folder moved into itself, a backend that is active, an upload whose bytes are all in) |
| `invalid` | `issue` (the reason of a refused configuration or the store), `kind=backend` and `id` when it is about a backend.add |
| `encryption-required` | `id` of the object the store refused |
| `stored-data-invalid` | `issue` (the reader's reason) |
| `backend-failed` | `kind=backend` when no object is named |
| `too-large` | `size`, `limit` in bytes (from `rpc`, for a chunk above the intake; a page whose cursor goes on has none) |

**Limits (no unbounded reading, no answer beyond a message).**
* Bytes: a `Blob.read` and a backend `get` answer at most 32 MiB (`clamp_read`; fewer is allowed, empty is the end). An `Upload.write` or `put` above
  16 MiB (`MAX_INTAKE_BYTES`) is `too-large` and writes nothing; the core streams the checksum, so a file of any size goes in by chunks. Tested with 34 MiB.
* Pages: the core cuts at 100 entries; a page is also checked against 32 MiB (`check_answer`). A cursor is the form of `rpc::Page` (version byte, place);
  a place beyond 2^32 is no cursor of ours and is refused (as an offset it would wrap to the start in the store).
* Texts: a name above 4096 bytes is the contract's `invalid-name` (the core decides the 255); a search text or key above 8 KiB, a media type, id or
  category above 1 KiB, all categories of one file above 64 KiB, a configuration above 64 KiB: `invalid-request`, before the core sees them.
* Reports: a `RescanReport` is held to 32 MiB; the lists share it (a short list is never crowded out) and **what is left out is named in `skipped`**
  (`... N more added entries (they are taken over) are not listed here`). `pendingUploads` and `BlobBackend.scan` are **paged** (since v0.35.0): 100 per page, oldest first / sorted by
  name. `pendingUploads` is cut in the store since 0.5.0 (the provider answers `pending_uploads_page`; a provider that only has the whole list gets the default cut); `scan` is still the whole listing of one folder cut by the server. `TrashReport.failed` lists at most 1 398 101 ids and `failedMissing` counts the rest.

**Places.** The binary refuses to start (exit 1, before anything is opened or made) if `FS_FILES_ROOT` is, or lies above, `--data`: `objects.db` would lie
among the files and a rescan would take it for one. Links are resolved first; a path with `..` is refused outright (after a part that does not exist yet it cannot
be resolved without making it). With that, a rescan cannot see the database; the core has no knowledge of its name and takes no special case.
`backend.add` over the socket is confined: the binary sets `FileRepo::with_local_base(<root>/.fs-files/backends)`, so a `local` configuration needs a path **relative to the
backends base** (absolute, `..`, a symbolic link on the way, the base itself, `.fs-files` in any case, and a folder inside another backend are `invalid` 422). The base lies in the
program's own folder, which a rescan never lists: a backend is no part of the main one (its files are not taken over, deleting a folder of the main backend does not reach it, and
a path like `Docs` makes a new folder there and covers nothing). **No two backends share a folder:** a folder is refused if another mounted backend has it, holds it or lies in it (both orders; `BlobBackend::local_root`, `BackendFactory::place`), and all of this is decided before anything is made. After a restart no backend is mounted, so a program may add its backends again under their ids. **Changed since a95acdf / 79e77c2:** a `local` configuration there took `path` relative to the root of the main backend and made the backend inside the tree of the main one; now a relative path such as `second` makes a new, empty folder under `.fs-files/backends`, and bytes of a backend added by those versions stay where they are (`<root>/second`) and are not taken over: such a backend has to be added again with that folder moved. Honest limits: the checks are by path, not `openat2`, and the base must be written by this program alone; `LocalFolder::open` makes
the folders it needs before the final check, so a link swapped in between the checks can leave empty folders behind (it still mounts nothing). The walk for a backend below the folder (`nested.rs`) is blocking `std::fs` work done while the repository lock is held; it is bounded, and moves to `spawn_blocking` later. **A backend that is not mounted is found on the disk:** a folder that has no program folder of its own is walked (depth 16, 10 000 entries, links not followed) and refused if a `.fs-files` lies anywhere below it, and also if the walk cannot finish (fail closed); so after a restart a parent of an earlier backend is refused, and the earlier one can be added again under its id. A failed add takes back exactly the folders that add made (and the program folder in any existing folder that had none), and does nothing through a link that took the place of the folder, nothing else, also after a failed check. Not handled: on file systems that fold case, `.FS-FILES` and `.fs-files` are one folder, which the name check treats as different spellings of the same name only for ASCII; other foldings (Unicode) are not compared. A program that embeds the core and takes `backend.add` from others should call `with_local_base` too; without it the core takes any path
(its own tests do). `s3` is not confined (a network address). A text of a request that is no UTF-8 is `invalid-request`. `examples/serve.rs` runs the server and a consumer in one process.

**Writes run to their end** (`run_to_end`): a client that drops the call or the connection cannot leave the disk and the database out of step. Calls on one `Upload` run in the
order they arrived, so chunks may be sent without waiting for each answer (mutation-tested).

**Open points of the server** (to the strategy session): (1) (closed by v0.35.0: problem types, paging, `failedMissing`); (2) `file.subscribe` sends the envelope of a
file marked sensitive (ADR 0029) like any other: the names and categories leave; whether they should only on release (as in `fs-people`) is a decision;
(3) `backend.add` is not persisted (the core's decision): after a restart the binary has the `main` local backend only; (4) the `Registry` of the feed is the
same code as in `fs-tasks` and `fs-maps` and wants to live once in `fs-interfaces` or `fs-objects`; (5) the cursor is a place in a list and shifts when an entry is
added between two pages (the shared limit of `fs-interfaces`); the pages of `list`/`find` are cut in the store with an offset, so a page costs the store a walk past the entries before it (rows read stay linear, measured), while a cursor as a key in the sort order (as in `fs-tasks`) would need one query shape per sort order (name, modified, size, type, both directions) and was not built; (5a) `BlobBackend.scan` reads the whole folder of the backend for every page (a file system has no sorted listing to continue from); `wrong-offset` and the upload errors from `write` carry no upload `id`; (6) the `Files` root has no way to hand out `BlobBackend`, correct for the contract, but a program that
wants remote backends needs the client side (`RemoteBackend`), not built; (7) a person who changes the folder from outside is seen only by `rescan`, no watcher.

## Crates

* `fs-objects` (v0.0.1), `fs-interfaces` (v0.0.18), `fs-components` (v0.0.8): git tags.
* `rustix` 1 (MIT OR Apache-2.0, Linux only, features `std` + `fs`): `renameat2` with `RENAME_NOREPLACE`, the atomic no-replace rename that `std` lacks. No C code with its default backend, already in the lock file through other crates.
* `blake3` 1.8 (CC0-1.0 OR Apache-2.0): checksums, with the `pure` feature (no C / assembly). Fast enough for a first version.
* `infer` 0.22 (MIT): media type from the first bytes when the caller gives none; pure Rust, no dependencies.
* `tokio` (fs, io, sync), `async-trait`, `thiserror`, `serde`, `serde_json`, `chrono` (MIT or Apache-2.0, already used by fs-objects / fs-people).
* `object_store` 0.14 (MIT OR Apache-2.0, Apache Arrow project), feature `aws` only, optional (`s3`): the S3 client with a real `MultipartStore`
  trait, which exposes the multipart id and the part etags. That is what a resumable upload needs (state is persisted, so a new process
  can go on). It also gives us signing, retries and the in-memory store for tests. Its TLS comes via `reqwest` + `rustls` + `aws-lc-rs`, which
  contains C code: not pure Rust. Hence the feature: `--no-default-features` is a pure-Rust build with the local folder only. `bytes` for its payloads.
* Server: `capnp` 0.27, `capnp-rpc` 0.27 (MIT), `tokio` (sync); `fs-interfaces` with feature `rpc` (socket, problems, pages, feed, `run_to_end`).
* Dev: `tempfile`, `uuid`, `async-trait`, `serde_json`, `chrono`, `blake3`.
* Not taken: `opendal` (broad, many services we do not use; its writer hides the multipart id, so an upload could not be resumed after a restart). `rusty-s3` + an own HTTP layer would be pure sans-IO but means writing the client.

## Contract

`files.capnp` in `fs-interfaces` (tag v0.0.18; draft 0.2; `files.capnp` itself is unchanged since v0.35.0, the steps after it are in `common.capnp` (`SupportReport.instance`) and in the problem extensions of `fs_interfaces::rpc`). Appended in v0.35.0: problem-type constants, `cursor` / `next` on `Transfer.pendingUploads` and `BlobBackend.scan`, `TrashReport.failedMissing`. Appended for real names: `Folders.rescan @13` and `RescanReport`; `BlobBackend`
`naming`, `publish`, `relocate`, `discard`, `makeDir`, `removeDir`, `scan` (@7 to @13) with `Spot`, `Scanned`, `ScanKind`; the problem types
`nameConflictProblemType` (409), `invalidNameProblemType` (422), `changedOutsideProblemType` (409); and the statement that `Envelope.name` of a
file and a folder IS its real name (not derived). In the core `Error::problem()` makes the mapping; a name clash is still `Error::Conflict`.
`OnConflict` is a last parameter of rename, move, copy and `startUpload` (0 = refuse, so old callers keep working; the wire `copyFile` now refuses by default, the Rust `copy_file` without `_as` still keeps both). `Sort` is a last parameter of `list` and `find`. Capabilities: `file.sort`, `file.conflictRule`, `folder.rescan` (Files), `blob.rename` and `blob.scan` (backend). Also appended since v0.8.0: `OnConflict` (v0.9.0), `Sort` (v0.10.0), size and date filters (v0.11.0), `emptyTrash`, `restoreTo`, `TrashReport` (v0.12.0), `getFolder` (v0.13.0), `pendingUploads`, `UploadInfo` (v0.14.0). `moveFolder`, `pending`, `FileFilter.trashed` exist since v0.5.0. `Files.subscribe @6` takes a `ChangeListener`
(common.capnp); the Rust form is `FileFeed` (the placeholder `Files::subscribe` is gone).

## Type descriptions and the window (ADR 0066, Thread R, 2026-11-10)

Types (crate `fs-files-types`, `DescribesType` of `fs-types` from `fs-interfaces` **v0.0.18**, the one `fs-admin-frontend` v0.0.8 (`fs-components`) uses; reported by `Files.types @7` of `files.capnp`):

| Type | Standard | Why |
|---|---|---|
| `File` | AS2 `Document` (narrowed to `Image`/`Video`/`Audio` by media type, as the core stores it) | a file is a document with bytes, a name and a media type |
| `Folder` | AS2 `Collection` | a collection of files and folders; the order is the view parameter `sortBy` of `find`, so not `OrderedCollection` |
| `StorageBackend` | AS2 `Service` (provisional, no standard for "a place that holds bytes") | `backend.list/add/remove` of the requirements are about it |
| `Picture` (gallery facet) | AS2 `Image` | refers to its `File`; the core has no metadata object yet |
| `Album` (gallery facet) | AS2 `Collection` | order is a view parameter (`album.sort`); items by reference |

Every command of the core and **every capability id of both `requirements.md`** is in a description (`requirements.rs` maps id to command; a test checks that none is missing). The list of a type is its `find` (empty = all, `Backend::list`; pages are read until the end, at most 5000 records). The app is the standard window shell with `fs-admin-frontend` (`fs-components` tag v0.0.8) over one `Backend` per type (`crates/fs-files-app/src/backend_*.rs`); the old workbench window (`source/`) is gone, its code is in git history. The file list and gallery grid components are in `fs-components` (branch `r2-files-components`, crates `fs-components-file-list`, `fs-components-gallery-grid`; **tag pending**), not used by the admin window; planned: they become the file view, a further frontend (see "One binary" below).

### Unsupported (named, not faked)

`CommandDescription.supported` exists since fs-interfaces v0.0.11. **`fs-files-types/src/support.rs` (`SUPPORTED`) is the one table** and fills it (`registry.rs`, `described`); `Backend::supported` answers from the same table, so the frontend shows the rest locked ("Soon"). A command not in it is unsupported, and `Backend::run` answers `BackendError::Unavailable("unsupported:<name>")` for it. The frontend is never changed to switch a control on; one row is added when the core gets the command. Not answered today (`cargo run -p fs-files-types --example unsupported`):

```text
Album: find create edit delete add remove nest query suggest sort           (no album object in the core; the list is empty)
File: uploadResume subscribe (a stream, not a one-shot command: the core has FileFeed) preview nameCheck renameBulk trashConfigure
      shareCreate shareRevoke shareRole versions revert pin encrypt syncConfigure tag duplicates searchContent link
      searchSave lock fileRequest undo
Folder: archive
Picture: register import libraryScan autoUpload metadataRead metadataEdit metadataExport tagAdd tagRemove previewGenerate
      open videoPlay edit delete restore duplicates merge stackCreate shareCreate shareGrant shareRevoke placeList
      placeAssign placeResolve sync personDetect personName personMerge personLink labelSuggest searchSemantic similar
      ocr favourite archive visibility commentAdd ratingSet batch story watermark transcode
StorageBackend: quota
```

Answered for `Picture`: `find` (= `media.list`, `media.search`: the image files), `download` (= `media.download`), `sensitiveMark` (= `classify`). `media.delete` / `media.restore` stay unsupported on purpose: they mean the metadata object, which the core does not have; trashing the file is `File.delete`.

### Provisional decisions and findings (for the strategy)

* Record ids and reference values are `urn:uuid:<uuid>` (a reference must be an absolute IRI for the basic type); the backend reads both forms. A move without the `folder`/`parent` field leaves the place unchanged, an empty list means the top level; `classify` without `sensitive` leaves the marking (rule for changing). A list command (kind `find`) with an input the core does not know (`searchContent` …) answers `unsupported:<key>`, never everything.
* No quantity type in `fs-types` yet: a size is decimal text (`size`, `minSize`, `maxSize`).
* A name that is taken has no `TypeProblem` code; it is reported as `choice.not-in-list` on the field `name` (the frontend shows the form finding). Other refusals of the core are `Unavailable(text)`.
* `upload` and `download` take a `file:` link (`BasicType::Link`) for the local path: the form has no file chooser (a component, the frontend's). A download never replaces an existing file.
* `Folder.create` / `move` and `Album` `parent` have a `reference` **without** a target, because a reference to the own type makes the frontend take the selected record for it (the rule for commands on a record) and the top level could not be chosen.
* `change` is partial in the backend (missing = unchanged, empty = cleared) for `classify`; `rename`/`move` send the fields they change.
* The generated frontend (v0.0.8) shows a locked "Soon" control for an unsupported command (checked in a picture of the window).
* The generated frontend has no paging and no tile or grid view for lists: a list is a table.
* Two `fs-types` versions in one build are separate types: this repo, `fs-admin-frontend` and `fs-interfaces` all use the one of v0.0.18 (fs-components v0.0.7 still pinned v0.0.17, so 0.0.7 of this program waited for fs-components v0.0.8).
* `CommandKind::Change` is the `Edit` of the type's own fields and has no input (`ChangeWithInput`). rename, move, classify and the like ask for input, so they go on the wire as `Other` (`registry.rs`); `Backend::change` answers `unsupported:change` and the window has no `change` form yet (open: map `Edit` to rename / move / classify). `Backend::run` gets the record's id next to the input (`with_record`).

## After the sprint (strategy list, 2026-11-07)

* `issue` values such as "empty or too long segment" (`backend/key.rs`, `backend/local/path.rs`) are sentences instead of identifiers (§2a): replace them by identifiers.
* The cursors of `list` and `find` are offsets; a cursor as a place in the sort key needs one query per sort order.
* `wrong-offset` and the upload errors from `write` carry no upload `id`; `name_taken` in `backend/local/io.rs` has no `name`.

## One binary `fs-files` and its window (ADR 0063, 2026-11-08)

`crates/fs-files` is the only binary: `fs-files` without arguments opens the window, `fs-files serve` runs the server (`fs_files_server::serve`; the folder the files live in is `FS_FILES_ROOT`), feature `gui` (default) builds the window. `fs-files-server` and `fs-files-app` are libraries. The window (`crates/fs-files-app`) is, since 0.0.8, the **file window**: the file workbench of `fs-components` (`FileManagerWidget`, crate `fs-components-workbenches`) over `RemoteFileSource` (`crates/fs-files-app/src/source/`, an Adapter of `FileSource` over `FilesRemote`), in the standard shell, with `Surface::status_line() -> false` (the workbench has its own status bar). It shows list or grid (`--view list|grid`), tabs, a path bar with search, a command bar, and a sidebar with Home, Pinned (the user folders of the person that the storage has, `xdg.rs` reads `user-dirs.dirs`), Storage with the folder tree, Network (nothing connected yet), the trash and the settings; columns can be hidden, moved and widened and the choice is kept (`FileStore`, ADR 0069; the real `Restorable` follows). The **generated Admin frontend** (`fs-admin-frontend`, ADR 0066 frontend "Admin") is behind `--admin` and `--facet admin` and stays over one `Backend` per data type (see "Type descriptions and the window"); the file window uses the file manager workbench of `fs-components` through `RemoteFileSource`, the Admin frontend does not. `FilesRemote` (a worker thread owns the Cap'n Proto connection) implements the `Backend` trait over `files.capnp`: folders, list and search, create, rename, move, copy, trash, restore, restore to, purge, empty trash and rescan, with the server's problems as the frontend's errors. **Done in 0.0.8 (decided 2026-11-11):** the generated Admin frontend stays; the file view of the workbench is a further frontend of Files (ADR 0066 lists several frontends per program) and the default window; the Admin frontend is reachable through `--admin`. This is no exception to ADR 0066. **Long calls off the interface thread (0.0.9):** the file workbench of fs-components v0.0.12 runs rescan, upload, download and resume on a worker (`FileJob`, `FilesState::finish`; see its DESIGN), `RemoteFileSource` stays the synchronous `FileSource` and is called from that worker; `FilesRemote` starts every call as a task of its own (`spawn_local`), so the short calls are answered while a transfer runs (`crates/fs-files-app/tests/nonblocking.rs`). **Known limit (0.0.9): the short calls (lists, tree, rename, move, copy, trash, preview) are still made on the interface thread**, each with its time limit (30 s; move, copy, restore and purge 15 min); a server that does not answer holds the window for that long. No abort, no progress: `files.capnp` has neither. `test-access` (optional feature, debug builds only, never in a release manifest) lets the typing test read what is typed into the fields; `gui::run` switches the registry on before the window is built. The window connects with the version handshake of `fs-interfaces` (see below). Tests: `crates/fs-files-app/tests/remote.rs` (real server, repository and folder), `crates/fs-files/tests/binary*.rs` (the real binary).

Not in the window yet (capabilities say so, nothing pretends): previews, upload, download, date filters, descending type sort; a file's sensitive marking (ADR 0029) is not read, so no entry shows one and the trash count says none is marked; classification, backends and sharing have no screen. Every call to the server has a time limit (see "Timeouts"). The Gallery facet is not built as a view of its own: `--facet gallery` shows the types Picture and Album only.


## Windows port (2026-11-10, sprint decision C: a real port, no cross-build, no exception)
**Pattern:** every part that differs between Linux and Windows is a trait with a Linux and a Windows implementation (ADR 0043):
| Part | Trait | Linux | Windows |
|------|-------|-------|---------|
| Names the local disk keeps | `NameRules` (`fs-files-core/src/platform`) | any name the shared rules accept | refuses `< > : " \| ? *`, a trailing dot or space and the device names (`CON`, `NUL`, `COM1`, also with an extension), because the disk would not refuse all of them (`a:b` is a stream of `a`); `same_name` ignores case, so a change of case only is renamed in place |
| Durability of a folder change | `Durability` | fsync of the folder | none (NTFS journals its metadata; a folder cannot be flushed) |
| Move without replacing | `Moves` | `renameat2(RENAME_NOREPLACE)` | the hard-link way (look, then rename for a folder) |
| Starting the server so that it outlives the window | `Detach` (`fs-files-app/src/detach`) | own process group | no console window, own process group |
| Reading a `file:` link | `FileLinks` (`fs-files-app/src/link`) | the text is the path | `file:///C:/Users/me/a.jpg` is `C:\Users\me\a.jpg` |
| Socket, lease and log files, ending in order | `fs-interfaces` `Endpoint` (named pipe), `companion_path`, `stop_self` | Unix socket, signal | named pipe, files in `%LOCALAPPDATA%\FreeSynergy\run`, no signal to send: the lease watcher calls `stop_self` |
A link planted in the root (symlink, or a junction on Windows) is refused the same way: `FileType::is_symlink` is true for both.
The lease lock needs a handle that can read or write data on Windows (it was `append`, which cannot lock there).
**Windows gate:** `.github/workflows/windows.yml` on `windows-2022` (fmt, clippy pedantic, tests per crate with a time limit, the coding rules).
Tests that depend on Unix modes (`0o700`, unreadable files) run on Unix only and say why; names that differ only in case are one name on
Windows and the tests say that. Until `fs-interfaces` is tagged with the transport, the crates point at its branch `win-port`.
**Open on Windows:** no path longer than the 4095 bytes counted by the repository is meant to be used (Windows takes more with the
`\\?\` form, which Explorer cannot open); UNC links (`file://server/share`, `file:////server/share`) and links whose decoded text holds a backslash (`%5C`) are refused, because the files of the app are local; the store's UAC/setup flow for fs-files itself is the one of `fs-store`.

## Q0 migration (0.0.6, 2026-11-11)

* **Blinc in one place.** No `blinc*` dependency, no `[patch]`/`[replace]`; the program hangs on `fs-components` only (one tag, now v0.0.8) (Blinc arrives through it; `fs_components::blinc` re-exports it, this code names no `blinc_*`). `.fs-rules.toml` has `blinc = "components-only"`; `check_blinc` of fs-release checks it. `fs-components-map` is not needed by this program.
* **Version handshake** (fs-interfaces DESIGN, "Version handshake"). The server answers `Control.version` (`run_until_signal_as`, `ServerIdentity::new("fs-files", version)`). The window connects with `FilesRemote::matching(socket, CommandStarter(current_exe, serve [--socket S] --lease L))`, which calls `connect_matching`: a server of this version is used, none is started, one of another version is stopped and replaced. A `HandshakeError` is kept as `BackendError::Handshake(text)`; the window opens anyway (descriptions from `fs-files-types` when the server cannot say them), every list shows the text and "Try again" calls `Backend::reconnect` = `FilesRemote::reconnect`. Never a black window. `FilesRemote::connect` (plain, no check, no start) stays for the tests. The server started by a window still ends when no window holds the lease. The log file of a server that stops at once is gone (a detached server has no stderr); the handshake error says it could not start. Tests: `crates/fs-files-app/tests/handshake*.rs`.
* **`--facet NAME`** is read by the one parser of fs-interfaces (`rpc::parse_facet`) in the window and in the server (`ServerOptions`); a name this program does not have is refused (`fs-files-types` `FACETS`: `files` = File, Folder, StorageBackend; `gallery` = Picture, Album). The server serves every type whatever the facet; the facet is the window's view.
* **`--version`** prints `fs-files <version>` (release smoke test).
* **Typing test** (`.github/workflows/typing-test.yml`, fs-release `actions/typing-test`): the release window runs under a virtual KWin with `--size 1000x640`; `.github/typing-test/focus.sh` clicks "Find" and then the "Name" field of the search form (positions from the window's corner); the 16 characters of the layout `de` are typed and checked. On GitHub the time is reported only. Locally: `scripts/typing-test.sh target/release/fs-files --size 1000x640` of fs-release with `TT_SCENARIO=.github/typing-test/focus.sh`.

## Timeouts (0.0.7, 2026-11-11)

* **Problem.** The window waited for ever on a server that passed the handshake and then said nothing: the four questions after the handshake (`folders`, `search`, `transfer`, `backends`), `describe` and every call from the window had no limit, so the window did not open or froze.
* **Now.** Every call goes through `fs_interfaces::rpc::client::call_limited("files", limit, call)` (fs-interfaces v0.0.18; the runtime of the worker has `enable_all`, so the timer works). `Limits` (`remote.rs`, `FilesRemote::matching_with` / `connect_with`): `start` 10 s for the questions after the handshake and for `describe` (the window is not open yet), `call` 30 s (`CALL_TIMEOUT`) for everything else, `long` 15 min for `upload`, `download`, `copy`, `move`, `restore`, `restoreTo`, `purge`, `emptyTrash` and `rescan` (they move file bytes or walk a tree: a large file over S3, a move between backends is a copy). The caller also waits at most `limit` plus 2 s on its channel.
* **What the person sees.** A timeout is `BackendError::Handshake("files server did not answer within ...")`: the one variant with a text and a "Try again" that connects anew. When the start questions hang, the remote is down with that text, the window opens anyway (descriptions from `fs-files-types`) and every list shows it. The timed-out call is dropped, the connection stays usable; a write may still have happened on the server (fs-interfaces says the same).
* **Tests.** `crates/fs-files-app/tests/silent.rs`: a fake `files::Server` that answers `Control.version` and then keeps silent (once at the start, once on `search.find`), with limits of 400 ms.

## Open: the sort of the file window (2026-11-12)

* The list of the file window evaluates ONE level of order (the first). The component reports Shift+click and the rank of the heading, but the contract (`files.capnp`, `Sort`) has one key and one direction; several levels need a list of `Sort` in `fs-interfaces`, a later common step (v0.0.19 stays as it is). Until then the window uses only the first level.
* Folders first is the order of this server since the file window (the contract has no flag for it yet; fs-interfaces v0.0.19 will carry `foldersFirst`, then the server takes the value of the client, default on). Before this change the server delivered folders and files mixed by name: a deliberate change of behaviour, named for the release notes.
* The type of a file is read from its first bytes, then from the ending of its name, then by whether it reads as text; `application/octet-stream` only when nothing fits.

## Lift to fs-components v0.0.11 (2026-11-12)

* `Cargo.toml` of the crates names `fs-components` `v0.0.11` and `fs-interfaces` `v0.0.19` (Blinc stays `fs-blinc-3.1`, the one source). These tags are local until they are pushed: build with `GIT_CONFIG_GLOBAL=/srv/FreeSynergy/.scratch/gitconfig-local-tags` and `CARGO_NET_GIT_FETCH_WITH_CLI=true` (a throw-away mapping, not committed). **Regenerate `Cargo.lock` after the tag push** (`cargo update -p fs-interfaces -p fs-types -p fs-components-app ...`), so that it names the sources on GitHub; until then the lock names the commits of the local tags (`0960ad83` and `90e62c30`).
* `test-access` is a debug-only feature (fs-components refuses it with `compile_error!` in a build without debug assertions). Here it is optional in `fs-files-app` and `fs-files`, off in every manifest of a release, and only the typing test (`.github/workflows/typing-test.yml`, a debug build) turns it on.
* The server reads `Sort.foldersFirst` (fs-interfaces v0.0.19): the client's value is respected, the default is on (an old client reads as true). Tested in `crates/fs-files-server/tests/search.rs`.
* `read_range` of the local backend opens with `O_NONBLOCK | O_NOFOLLOW` and checks the type of what was opened (a regular file), instead of trusting the path: a FIFO put in the place of a file no longer holds a thread for ever, and a link is not followed. Tested (`backend/local/io.rs`: the FIFO test is red without the change, by its timeout).
