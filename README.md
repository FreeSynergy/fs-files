# fs-files (FreeSynergy.Files)

FreeSynergy program (ADR 0017): files and folders. What we know about a file is an object in
[fs-store](https://github.com/FreeSynergy/fs-store); its bytes are in a backend: a local folder (default) or
S3 (RustFS and others). Other programs refer to a file by id and never see paths or backends.

The program core is `crates/fs-files-core` (no GUI). Contract: `files.capnp` in
[fs-interfaces](https://github.com/FreeSynergy/fs-interfaces). See `DESIGN.md`. Licence: MIT OR Apache-2.0.
