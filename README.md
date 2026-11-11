# FreeSynergy Files — keep and find anything

**Your files stay where you put them, under the names you gave them.**

FreeSynergy Files looks after files and folders. It knows what each file is, how big it is and where it lives, so other programs can refer to a file without ever seeing a path. The bytes can sit in a plain folder on your disk, or in S3-compatible storage such as RustFS.

What is different here: with the default local backend your files lie in an ordinary folder under their real names, for example `Urlaub/Foto.jpg`. You can open that folder with any other tool. Deleting goes through a trash first. A name clash never overwrites silently: you refuse it or keep both. Uploads survive a restart and pick up where they stopped. Other programs are meant to refer to a file by its id and never see a path or a backend, so one file does not need to be copied into each program.

## What it can do today

Version 0.0.7 (in progress). Latest tag: `v0.0.6`.

- Folders, rename, move, copy, trash, restore, purge, and an empty-trash call.
- Resumable uploads with a checksum computed while the bytes arrive.
- A local-folder backend with real names, and an S3 backend (tested once against a real RustFS server, not part of the automatic tests).
- Search, sorting by name, date, size or type, and filters by size and date.
- "Rescan": take over files that someone put into the folder from outside. It deletes nothing.
- Disk and database are kept in step: if one step fails, the other is undone.
- Files with sensitive categories are marked, and the store refuses them unencrypted.
- A small server that offers all of this over a local socket.
- Tests: 213 run by default.

## What is not there yet

- The window is the generated Admin window (`fs-files`, `fs-files --facet gallery` for pictures and albums); previews, a gallery grid and a music or video view are planned. Planned (decided 2026-11-11): the file view becomes a further frontend and later the default window; the Admin window stays reachable through the facet or a switch.
- No sharing (the interface is an empty placeholder), no versions, no encryption of file contents, no sync, no WebDAV, no previews.
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
