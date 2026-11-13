# FreeSynergy Files — keep and find anything

**Your files stay where you put them, under the names you gave them.**

FreeSynergy Files looks after files and folders. It knows what each file is, how big it is and where it lives, so other programs can refer to a file without ever seeing a path. The bytes can sit in a plain folder on your disk, or in S3-compatible storage such as RustFS.

What is different here: with the default local backend your files lie in an ordinary folder under their real names, for example `Urlaub/Foto.jpg`. You can open that folder with any other tool. Deleting goes through a trash first. A name clash never overwrites silently: you refuse it or keep both. Uploads survive a restart and pick up where they stopped. Other programs are meant to refer to a file by its id and never see a path or a backend, so one file does not need to be copied into each program.

## What it can do today

Version 0.0.10 (in progress). Latest tag: `v0.0.9`. What changed: [CHANGELOG.md](CHANGELOG.md) (English and German).

- A file window (the standard window): list with columns you can hide, move and widen, or a grid with big icons, tabs, a path bar with search, a command bar, a sidebar with your pinned user folders, the storage and the trash, and a status bar. The generated Admin frontend stays behind `--admin`.
- **The face is chosen in the window (0.0.10).** Menu View: `Face: List` or `Face: Grid`; the choice is kept in `files-window.conf` in the data folder (a damaged file is the list). `--view` overrides it for one start. **Honest limit:** there is one face with two ways to show a folder; a second face does not exist yet.
- Folders come first in every listing (a deliberate change: before, folders and files were mixed by name), and the type of a file is read from its first bytes, then its ending, then whether it reads as text.
- Folders, rename, move, copy, trash, restore, purge, and an empty-trash call.
- Resumable uploads with a checksum computed while the bytes arrive.
- A local-folder backend with real names, and an S3 backend (tested once against a real RustFS server, not part of the automatic tests).
- Search, sorting by name, date, size or type, and filters by size and date.
- "Rescan": take over files that someone put into the folder from outside. It deletes nothing.
- Disk and database are kept in step: if one step fails, the other is undone.
- Files with sensitive categories are marked, and the store refuses them unencrypted.
- A small server that offers all of this over a local socket.
- Tests: `cargo test --workspace` (263 at version 0.0.10; two live S3 tests are ignored).

## What is not there yet

- The standard window is the file window (list or grid, tabs, path bar, command bar, sidebar with Home, Pinned user folders, Storage, Network and the trash). The generated Admin window is behind `--admin` (and `--facet admin`; `--facet files` and `--facet gallery` show only the types of one facet). Planned and not there: previews and thumbnails, a gallery grid, a music or video view, cut, copy and paste, more than one tab, sorting by more than one column.
- **Long calls do not freeze the window (0.0.9).** Rescan, upload and download run on a worker (fs-components v0.0.13): the window answers, shows "Running...", a second start is refused, a failed call gives the input back; no abort or progress (the server has none). **Limit:** lists, tree, rename, move, copy, trash and preview are still called on the interface thread, each with a time limit (30 s; move, copy, restore and purge 15 min).
- No sharing (the interface is an empty placeholder), no versions, no encryption of file contents, no sync, no WebDAV, no previews, no second storage in the sidebar (only the own storage; `LocalFolder` of the home folder or a USB stick is planned).
- Replacing a file is not offered, only refuse or keep both.
- The interface is a draft marked provisional, and nothing uses it yet.

## Part of FreeSynergy

Files is meant to be the shelf the other programs put things on. In the plan it also feeds the gallery, video and music views and streaming. FreeSynergy is a family of programs that are free for everyone, forever, and that keep your data yours. The documentation, the manifesto and the overall plan live in [fs-documentation](https://github.com/FreeSynergy/fs-documentation); the reading version is at <https://freesynergy.github.io/fs-documentation/>. Start with the [manifesto](https://freesynergy.github.io/fs-documentation/manifesto/) if you want to know why we do this.


## For developers

- Workspace crates: `fs-files-core` (core, no GUI), `fs-files-server` (contract over a Unix socket).
- The contract is `files.capnp` in [fs-interfaces](https://github.com/FreeSynergy/fs-interfaces) (draft, provisional). Files owns it.
- What we know about a file is an object in [fs-objects](https://github.com/FreeSynergy/fs-objects) (the object store).
- Build and test: `cargo build --workspace` and `cargo test --workspace`. The live S3 tests are ignored by default.
- Design and the full state table: [DESIGN.md](DESIGN.md).
- Licence: MIT OR Apache-2.0.
