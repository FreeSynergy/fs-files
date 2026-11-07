# fs-files – Design

`fs-files` is the files program of FreeSynergy. This repo holds the **program core** (`crates/fs-files-core`, no GUI), the **server** (`crates/fs-files-server`, the contract over a Unix socket), the **client** (`crates/fs-files-client`, the traits of the core over that socket) and the **app** (`crates/fs-files-app`, the window; talks to the server only).
**Metadata** (name, media type, size, checksum, folder, trash flag) is an object in `fs-store`; the **bytes** are in a
backend behind `BlobBackend`. Other programs refer to a file by its object id and read through the `Blob` trait; they never
see a path or a backend. Contract: `files.capnp` in `fs-interfaces` v0.35.0 (draft, consumers: the app, 1 of 2).

## State

Version 0.4.0, not tagged (stage 1: local folder, stage 2: S3, stage 3: real names, stage 4: the server). Built on fs-store 0.2.0 and fs-interfaces 0.35.0 (git tags). Interface status: draft, provisional, consumers 0 of 2.

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
| Client `fs-files-client`: `RpcFiles` implements `Files` and `Transfer` over the socket of `fs-files-server` (every method the app calls and the plain forms of them); the rest is `Unsupported`; see Client | done, draft |
| Frontend `fs-files-app`: the workbench `file-manager` of `fs-components` over the socket (`--socket`); holds no store and no backend; see Frontend | first version, provisional; reviewed once, 2026-11-07 (Opus), see Frontend |
| Tests | run by default: core 141, server 60, client 22, app 50 (incl. unit tests); 4 ignored (2 live S3 in the core, 2 measurements in the app). The server tests run over a real socket against a real `FileRepo`; the client and app tests run the real server on a thread of its own with a real folder and the real client; the app also runs its real binary |

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
| `fs-files-client/{peer,remote}.rs` | Adapter | `RpcFiles` hides the connection thread (the shared `Worker` of `fs-interfaces`); `remote.rs` fetches the capabilities at connect |
| `fs-files-client/{files_impl,transfer_impl}.rs` | Adapter | the traits `Files` and `Transfer` of the core, one call each; the upload and the blob handed out are small adapters that name their upload or file again in every call |
| `fs-files-client/{folder,file,transfer}_calls.rs`, `read.rs`, `write.rs`, `cursor.rs` | Adapter | requests and replies of the contract as values of the core; pages and the unfinished uploads are read page by page |
| `fs-files-client/problems.rs` | Adapter | the table of problem types to `fs_files_core::Error`, chosen by the type, never by the words |
| `fs-files-app/src/source/{adapter,provider,listing,change}.rs` | Adapter | the contract's `Files` and `Transfer` as the workbench's `FileSource`; a request of the workbench as one call |
| `fs-files-app/src/source/transfer.rs` | Facade | upload, resume, drop and download in front of the client and the person's file system: one fixed flow each, and it reads, writes and calls |
| `fs-files-app/src/source/paths.rs` | Specification | what a path of the person's own computer may be |
| `fs-files-app/surface.rs` | Adapter, Command | the workbench as a screen of the shell; a menu entry is an id and one handler |
| `fs-files-app/args.rs`, `words.rs` | none | a pure function over the argument list; constants (whole sentences, named places, ADR 0057) and pure functions that fill them: nothing is read, written or called |
| `fs-files-app/main.rs` | none | a thin binary: parses, connects, hands over; no logic of its own |
| `fs-files-app/commands.rs` | Facade | two small functions (upload, download) in front of the `FileSource`; it calls it, so it is not `none` |
| `fs-files-app/tests/common/decorators.rs` | Decorator | a provider that misbehaves on purpose in front of the real `FileRepo` (a hostile name; the server stopped after n chunks) |

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

**Limits (no unbounded reading, no answer beyond a message).**
* Bytes: a `Blob.read` and a backend `get` answer at most 32 MiB (`clamp_read`; fewer is allowed, empty is the end). An `Upload.write` or `put` above
  16 MiB (`MAX_INTAKE_BYTES`) is `too-large` and writes nothing; the core streams the checksum, so a file of any size goes in by chunks. Tested with 34 MiB.
* Pages: the core cuts at 100 entries; a page is also checked against 32 MiB (`check_answer`). A cursor is the form of `rpc::Page` (version byte, place);
  a place beyond 2^32 is no cursor of ours and is refused (as an offset it would wrap to the start in the store).
* Texts: a name above 4096 bytes is the contract's `invalid-name` (the core decides the 255); a search text or key above 8 KiB, a media type, id or
  category above 1 KiB, all categories of one file above 64 KiB, a configuration above 64 KiB: `invalid-request`, before the core sees them.
* Reports: a `RescanReport` is held to 32 MiB; the lists share it (a short list is never crowded out) and **what is left out is named in `skipped`**
  (`... N more added entries (they are taken over) are not listed here`). `pendingUploads` and `BlobBackend.scan` are **paged** (since v0.35.0): 100 per page, by weight, oldest first / sorted by
  name, the cursor of `rpc::Page`; the provider returns the whole list and the server cuts it. `TrashReport.failed` lists at most 1 398 101 ids and `failedMissing` counts the rest.

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
same code as in `fs-tasks` and `fs-maps` and wants to live once in `fs-interfaces` or `fs-store`; (5) the cursor is a place in a list and shifts when an entry is
added between two pages (the shared limit of `fs-interfaces`); (6) the `Files` root has no way to hand out `BlobBackend`, correct for the contract, but a program that
wants remote backends needs the client side (`RemoteBackend`), not built; (7) a person who changes the folder from outside is seen only by `rescan`, no watcher.

## Data-loss paths

What could lose or misplace bytes, and what stops it. **Download:** a file is written whole or not at all, never over an existing path or a link; a marked file only with the person's release and readable by its owner only; the name comes from the server, so it is checked with `check_name` before it is joined to the path (`../x`, `/abs`, `a/b` are refused, tested with a decorator in front of the real server). A file above `FileAdapter::MOST_IN_MEMORY` (2 GiB) is refused before anything is written (tested with a smaller limit; the real value is asserted). **Upload:** chunks go in order; a connection that breaks leaves what arrived as an unfinished upload, no half file is made (tested by stopping the real server after two chunks, then resuming on the provider). **Resume:** the core cannot tell whether a file is the one an upload started from; the app checks only that the name matches (a number added by keep-both is allowed) and that the file is not smaller than what arrived. A different file of the same name and a larger size is appended. That is a known gap, not closed.

## Client (`crates/fs-files-client`)

`RpcFiles` is the Adapter of `Files` and `Transfer` in front of the Cap'n Proto client of `files.capnp`, so a frontend depends on the traits and never on the
core (ADR 0021). `RpcFiles::connect(socket)` (or `connect_with(socket, Timeouts)`) starts the shared `Worker<FilesRemote>` of `fs_interfaces::rpc::client`: one
thread with a current-thread runtime and a `LocalSet` (a capnp-rpc client is not `Send`); `FilesRemote::open` asks `supportReport` and takes `Folders`,
`Transfer` and `Search`; calls from any thread queue for that thread and overlap there. **Deadlines:** the handshake waits at most 5 s, a call at most 30 s
(`files server did not answer within <limit>`); a write that ran out of time may still have happened, the server finishes what it began. A server that is
gone is `Error::Backend("files server: the connection is gone")` on every later call; there is **no reconnect** (restart the app).

| Method | Over the wire |
|---|---|
| `capabilities` | what `supportReport` said at connect, cut to what the client calls (a snapshot) |
| `get`, `get_folder` | `Folders.get`, `getFolder`; `found = false` is `None` |
| `list`, `list_sorted`, `find`, `find_sorted` | `Folders.list`, `Search.find`: one page per call, the cursor of the wire is the core's `Cursor` |
| `create_folder`, `rename_folder[_as]`, `move_folder[_as]`, `delete_folder` | `Folders.createFolder`, `renameFolder`, `moveFolder`, `deleteFolder` |
| `move_file[_as]`, `copy_file[_as]`, `rename_file[_as]` | `moveFile`, `copyFile`, `renameFile`; the plain forms are the refuse rule (`copy_file`: keep both, as in the core) |
| `trash`, `restore[_as]`, `restore_to`, `purge`, `empty_trash`, `rescan` | the same-named calls |
| `start_upload[_as]`, `resume_upload`, `pending_uploads`, `open` | `Transfer.startUpload`, `resumeUpload`, `pendingUploads` (every page until the cursor is empty), `open` |
| `Upload::{write, received, finish, abort}`, `Blob::{size, read}` | `Upload.*`, `Blob.*` |

**Handles are ids.** A capability of the wire cannot leave the thread of the connection, so an `RpcUpload` is the id of the session and an `RpcBlob` the id of
the file, and each call names its upload (`resumeUpload`) or file (`open`) again. That costs one more round trip per call on a local socket and gives: a handle
survives a dropped one, nothing leaks on the server, and an upload goes on after a lost connection exactly as the contract wants. `start_upload` and
`resume_upload` check once that the session exists. A chunk is what the caller gives (the app uses `CHUNK`, 1 MiB; the server takes 16 MiB per write and
answers at most 32 MiB per read).

**Unsupported (explicit):** `classify` (`file.classify`), `Files::sharing`, the `Backends` interface (`backend.list`, `backend.add`, `backend.remove`), `subscribe` /
`FileFeed` (`file.subscribe`) and `BlobBackend` (the provider side) are not implemented: the trait defaults answer `Unsupported`. The server offers the first
four; the client leaves them out because the app calls none of them. **`capabilities()` is cut to match:** those ids are reported as not possible ("the server
offers it, the client of the files program does not yet"), everything else as the server said (`sharing` and the like: the server's own `Unsupported`).

**Errors** are read back from the problem type (the wire carries a type, a status and a text, not the fields of a variant): `name-conflict` is `Conflict`
(the text begins `name taken: `, so `Error::problem()` gives the same type again), `invalid-name` is `InvalidName`, `changed-outside` is `Backend` with
`changed outside: ` in front (again `problem()` gives the type), `not-found` is `NotFound(text)`, `conflict` is `Conflict`, `wrong-offset` is `Offset { expected, got }`
(the two numbers are read from the text; a text without them stays a `Conflict`), `invalid` is `Config`, `encryption-required` is `Store(EncryptionRequired(id))` when the text names the
id, `stored-data-invalid` is `Store(Backend(..))` (there is no way to build a `Json` error from the wire), `backend-failed` is `Backend`, `invalid-request` and `too-large` are `Store(Invalid)`, `unsupported` is
`Unsupported(capability, reason)`. **Deviations from the core:** the id of a not-found is not carried (the text is); a change of wording on the server breaks the reading of
`Offset` and the ids (the table is tested in `problems_table.rs`, the common types over the real socket). `TrashReport.failedMissing` (ids the server could not list,
only with more than 1.3 million failures) has no place in the core's type and is dropped.

**What does not arrive exactly:** `published` and `updated` of the envelope come in milliseconds, the wire's resolution (the store's microseconds are lost).

**Tests** (`tests/{folders,paging,transfer,rescan,gone}.rs`, `problems_table.rs`, `cursor.rs`): the real server on a Unix socket on a thread of its own, the real `FileRepo` with a real
folder, the real client; values compared with the provider behind the server; 130 folders over two pages with the cursors equal to the provider's; 105 unfinished uploads over two pages;
a 3 MiB upload in chunks, resumed with a new handle, a wrong offset writes nothing; ranges of a blob; a server that stops. Not tested: an upload above the server's limits over the
wire (the server's own tests cover 34 MiB), a server that never answers (the shared worker's tests cover the deadline).

## Frontend (`crates/fs-files-app`, provisional, reviewed once)

The work surface is the workbench `file-manager` of `fs-components` (it knows no store and no type of this repo); the app is the adapter between it and the contract, plus the
shell with its menus (`surface.rs`: File: New folder, Upload, Rescan; View: List, Grid, Trash, Transfers; only what the source can do is offered, nothing destructive), the command line
and the words (`words.rs`). It imports no `blinc_*` crate. **`--socket <path>` is required** and names a running `fs-files-server`; `--root` and `--store` are refused with a hint at the
server (the folder is its `FS_FILES_ROOT`, the store its `--data`). The app opens no store and no backend. Further options: `--no-rescan`, `--snapshot`, `--size`, `--view list|grid`,
`--upload`, `--into`, `--keep-both`, `--download`, `--to`, `--release-sensitive`. A server that is not there is `cannot reach the files server at <path>: ...` and exit code 1.
Pinned: `fs-interfaces` v0.35.0 and `fs-components` at `8c38a3b` (a commit of `main`, not yet a tag).

**What works** (all of it over the socket, tested in `crates/fs-files-app/tests` against the real server): folders as a tree, breadcrumb, list and grid; sorting by name, size, type (ascending only) and
changed; search by name and filter by size and date (`find` has no "top level only", so a page can be short); create, rename, move, copy folders and files, with the keep-both rule; trash, restore (also to
another folder), purge, empty; rescan (also at the start); upload and download by a path typed in (chunked, resumable, a list of unfinished uploads to resume or drop, a taken name refused or kept both,
a file marked sensitive written only with the person's release and readable by its owner only); the preview of the start of a file; marks of sensitive files shown (never opened by the workbench).

**Paging and the "+1" effect.** The workbench does not ask by offset: it hands the source the page mark of the last page and loops until it has its goal of rows (`PAGE`, 200). The mark is the
core's `Cursor`, i.e. the cursor of `Folders.list` and `Search.find`; the unfinished uploads are read page by page in the client. So the page size is the server's (100), the workbench takes
what comes, and there is no re-reading from the start to skip rows as in `fs-people-app`. The cursor is a place in the list and shifts if an entry is added between two pages (the limit of `fs-interfaces`).

**`Unsupported` / not there, honestly:**
* **Dates:** the entries show none (`FileCapabilities::dates = false`), although the envelope has `updated`; the adapter does not pass it on yet (a review point). Sorting by "changed" works.
* **Sort by type descending:** the contract says ascending only.
* **`subscribe`:** the app has none; a change another program or a person makes in the folder shows after the next read or Rescan. The client does not implement `FileFeed`.
* **Marking a file sensitive** (`classify`), **backends** (`Backends`, S3 and others): not in the app and not in the client. **Sharing, versions, encryption, sync, WebDAV, tags:** not built in the contract either.
* **Upload and download are paths typed into the input (and the command line):** there is no file dialog (it needs Blinc support). Both **block the interface thread** until they are done, per call with the
  30 s deadline of the client (a chunk is one call; a large file is many); there is no worker that posts back, so the window does not move meanwhile. A download is written whole from memory and refused above 2 GiB. **Bigger than that over the wire is not
  built; there is no stand-in.** The transfer list shows no sizes and the command line prints no byte counts: the commits of the workbench that return them (`Transferred`, branch `files-gui`, not pushed to `fs-components`) are not in `main`, so the
  app is on `main`'s API (`upload` and `download` answer the name).
* **A `rescan` of a very large folder** is one call under the 30 s deadline; one that is missed shows as an error while the server goes on.
* **A server that stops** is an error on the next action; no reconnect.
* **Reviewed once, 2026-11-07 (Opus): no data loss found.** Fixed after it: patterns named as in `coding.md` 2, sentences with named places in `words.rs`, the name of a downloaded file checked with `check_name` (a hostile server cannot choose the path), tests for the 2 GiB limit and for a connection that breaks mid-upload. Remaining points: dates in the entries, a file dialog, work off the interface thread, `subscribe`, the resume gap (see "Data-loss paths"), all named above. The measurements `tests/measure.rs` (large folder, large upload) are ignored and were not run over the socket.

## Crates

* `fs-store` (v0.2.0), `fs-interfaces` (v0.35.0): git tags.
* `rustix` 1 (MIT OR Apache-2.0, Linux only, features `std` + `fs`): `renameat2` with `RENAME_NOREPLACE`, the atomic no-replace rename that `std` lacks. No C code with its default backend, already in the lock file through other crates.
* `blake3` 1.8 (CC0-1.0 OR Apache-2.0): checksums, with the `pure` feature (no C / assembly). Fast enough for a first version.
* `infer` 0.22 (MIT): media type from the first bytes when the caller gives none; pure Rust, no dependencies.
* `tokio` (fs, io, sync), `async-trait`, `thiserror`, `serde`, `serde_json`, `chrono` (MIT or Apache-2.0, already used by fs-store / fs-people).
* `object_store` 0.14 (MIT OR Apache-2.0, Apache Arrow project), feature `aws` only, optional (`s3`): the S3 client with a real `MultipartStore`
  trait, which exposes the multipart id and the part etags. That is what a resumable upload needs (state is persisted, so a new process
  can go on). It also gives us signing, retries and the in-memory store for tests. Its TLS comes via `reqwest` + `rustls` + `aws-lc-rs`, which
  contains C code: not pure Rust. Hence the feature: `--no-default-features` is a pure-Rust build with the local folder only. `bytes` for its payloads.
* Client: `fs-interfaces` with feature `rpc` (the shared `Worker`, `Timeouts`, problems), `capnp` 0.27, `async-trait`; `fs-files-core` without the S3 feature.
* App: `fs-components-app`, `-workbenches`, (tests) `-render`, `-widgets` at `8c38a3b` of `fs-components`; `chrono`, `tokio` (runtime for the workbench's blocking calls); dev: `image`, `tempfile`, the server.
* Server: `capnp` 0.27, `capnp-rpc` 0.27 (MIT), `tokio` (sync); `fs-interfaces` with feature `rpc` (socket, problems, pages, feed, `run_to_end`).
* Dev: `tempfile`, `uuid`, `async-trait`, `serde_json`, `chrono`, `blake3`.
* Not taken: `opendal` (broad, many services we do not use; its writer hides the multipart id, so an upload could not be resumed after a restart). `rusty-s3` + an own HTTP layer would be pure sans-IO but means writing the client.

## Contract

`files.capnp` in `fs-interfaces` v0.35.0 (draft 0.2). Appended in v0.35.0: problem-type constants, `cursor` / `next` on `Transfer.pendingUploads` and `BlobBackend.scan`, `TrashReport.failedMissing`. Appended for real names: `Folders.rescan @13` and `RescanReport`; `BlobBackend`
`naming`, `publish`, `relocate`, `discard`, `makeDir`, `removeDir`, `scan` (@7 to @13) with `Spot`, `Scanned`, `ScanKind`; the problem types
`nameConflictProblemType` (409), `invalidNameProblemType` (422), `changedOutsideProblemType` (409); and the statement that `Envelope.name` of a
file and a folder IS its real name (not derived). In the core `Error::problem()` makes the mapping; a name clash is still `Error::Conflict`.
`OnConflict` is a last parameter of rename, move, copy and `startUpload` (0 = refuse, so old callers keep working; the wire `copyFile` now refuses by default, the Rust `copy_file` without `_as` still keeps both). `Sort` is a last parameter of `list` and `find`. Capabilities: `file.sort`, `file.conflictRule`, `folder.rescan` (Files), `blob.rename` and `blob.scan` (backend). Also appended since v0.8.0: `OnConflict` (v0.9.0), `Sort` (v0.10.0), size and date filters (v0.11.0), `emptyTrash`, `restoreTo`, `TrashReport` (v0.12.0), `getFolder` (v0.13.0), `pendingUploads`, `UploadInfo` (v0.14.0). `moveFolder`, `pending`, `FileFilter.trashed` exist since v0.5.0. `Files.subscribe @6` takes a `ChangeListener`
(common.capnp); the Rust form is `FileFeed` (the placeholder `Files::subscribe` is gone).
