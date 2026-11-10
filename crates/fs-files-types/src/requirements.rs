//! The capabilities of the two `requirements.md` files (Files and Gallery) and the command that carries each.
//!
//! Pattern: Specification (`coding.md` §2) — a fixed table; a test checks that every row names a described command and
//! that nothing of the requirements is missing. Ids with a `.gallery` suffix are the gallery's own spelling of an id the
//! files list also has (`share.create`, `share.revoke`).

/// One capability of a requirements list and where it lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Requirement {
    /// The id as written in `requirements.md`.
    pub id: &'static str,
    /// The data type that carries it.
    pub data_type: &'static str,
    /// The command of that type.
    pub command: &'static str,
}

/// All of them.
pub const REQUIREMENTS: &[Requirement] = &[
    Requirement {
        id: "folder.list",
        data_type: "File",
        command: "find",
    },
    Requirement {
        id: "folder.list",
        data_type: "Folder",
        command: "find",
    },
    Requirement {
        id: "folder.create",
        data_type: "Folder",
        command: "create",
    },
    Requirement {
        id: "folder.rename",
        data_type: "Folder",
        command: "rename",
    },
    Requirement {
        id: "folder.delete",
        data_type: "Folder",
        command: "delete",
    },
    Requirement {
        id: "file.upload",
        data_type: "File",
        command: "upload",
    },
    Requirement {
        id: "file.download",
        data_type: "File",
        command: "download",
    },
    Requirement {
        id: "file.move",
        data_type: "File",
        command: "move",
    },
    Requirement {
        id: "file.copy",
        data_type: "File",
        command: "copy",
    },
    Requirement {
        id: "file.rename",
        data_type: "File",
        command: "rename",
    },
    Requirement {
        id: "file.delete",
        data_type: "File",
        command: "delete",
    },
    Requirement {
        id: "file.restore",
        data_type: "File",
        command: "restore",
    },
    Requirement {
        id: "file.search",
        data_type: "File",
        command: "find",
    },
    Requirement {
        id: "file.preview",
        data_type: "File",
        command: "preview",
    },
    Requirement {
        id: "file.resolve",
        data_type: "File",
        command: "resolve",
    },
    Requirement {
        id: "backend.list",
        data_type: "StorageBackend",
        command: "find",
    },
    Requirement {
        id: "backend.add",
        data_type: "StorageBackend",
        command: "create",
    },
    Requirement {
        id: "backend.remove",
        data_type: "StorageBackend",
        command: "delete",
    },
    Requirement {
        id: "file.classify",
        data_type: "File",
        command: "classify",
    },
    Requirement {
        id: "subscribe",
        data_type: "File",
        command: "subscribe",
    },
    Requirement {
        id: "name.check",
        data_type: "File",
        command: "nameCheck",
    },
    Requirement {
        id: "file.renameBulk",
        data_type: "File",
        command: "renameBulk",
    },
    Requirement {
        id: "trash.purge",
        data_type: "File",
        command: "purge",
    },
    Requirement {
        id: "trash.configure",
        data_type: "File",
        command: "trashConfigure",
    },
    Requirement {
        id: "backend.quota",
        data_type: "StorageBackend",
        command: "quota",
    },
    Requirement {
        id: "upload.list",
        data_type: "File",
        command: "uploadList",
    },
    Requirement {
        id: "upload.abort",
        data_type: "File",
        command: "uploadAbort",
    },
    Requirement {
        id: "share.create",
        data_type: "File",
        command: "shareCreate",
    },
    Requirement {
        id: "share.revoke",
        data_type: "File",
        command: "shareRevoke",
    },
    Requirement {
        id: "file.versions",
        data_type: "File",
        command: "versions",
    },
    Requirement {
        id: "file.revert",
        data_type: "File",
        command: "revert",
    },
    Requirement {
        id: "file.encrypt",
        data_type: "File",
        command: "encrypt",
    },
    Requirement {
        id: "sync.configure",
        data_type: "File",
        command: "syncConfigure",
    },
    Requirement {
        id: "file.tag",
        data_type: "File",
        command: "tag",
    },
    Requirement {
        id: "file.duplicates",
        data_type: "File",
        command: "duplicates",
    },
    Requirement {
        id: "file.searchContent",
        data_type: "File",
        command: "searchContent",
    },
    Requirement {
        id: "file.pin",
        data_type: "File",
        command: "pin",
    },
    Requirement {
        id: "file.link",
        data_type: "File",
        command: "link",
    },
    Requirement {
        id: "search.save",
        data_type: "File",
        command: "searchSave",
    },
    Requirement {
        id: "file.lock",
        data_type: "File",
        command: "lock",
    },
    Requirement {
        id: "file.shareRole",
        data_type: "File",
        command: "shareRole",
    },
    Requirement {
        id: "file.fileRequest",
        data_type: "File",
        command: "fileRequest",
    },
    Requirement {
        id: "file.undo",
        data_type: "File",
        command: "undo",
    },
    Requirement {
        id: "file.archive",
        data_type: "Folder",
        command: "archive",
    },
    Requirement {
        id: "folder.rescan",
        data_type: "Folder",
        command: "rescan",
    },
    Requirement {
        id: "media.register",
        data_type: "Picture",
        command: "register",
    },
    Requirement {
        id: "media.import",
        data_type: "Picture",
        command: "import",
    },
    Requirement {
        id: "library.scan",
        data_type: "Picture",
        command: "libraryScan",
    },
    Requirement {
        id: "media.autoUpload",
        data_type: "Picture",
        command: "autoUpload",
    },
    Requirement {
        id: "metadata.read",
        data_type: "Picture",
        command: "metadataRead",
    },
    Requirement {
        id: "metadata.edit",
        data_type: "Picture",
        command: "metadataEdit",
    },
    Requirement {
        id: "tag.add",
        data_type: "Picture",
        command: "tagAdd",
    },
    Requirement {
        id: "tag.remove",
        data_type: "Picture",
        command: "tagRemove",
    },
    Requirement {
        id: "preview.generate",
        data_type: "Picture",
        command: "previewGenerate",
    },
    Requirement {
        id: "media.list",
        data_type: "Picture",
        command: "find",
    },
    Requirement {
        id: "media.open",
        data_type: "Picture",
        command: "open",
    },
    Requirement {
        id: "video.play",
        data_type: "Picture",
        command: "videoPlay",
    },
    Requirement {
        id: "media.search",
        data_type: "Picture",
        command: "find",
    },
    Requirement {
        id: "album.create",
        data_type: "Album",
        command: "create",
    },
    Requirement {
        id: "album.edit",
        data_type: "Album",
        command: "edit",
    },
    Requirement {
        id: "album.delete",
        data_type: "Album",
        command: "delete",
    },
    Requirement {
        id: "album.add",
        data_type: "Album",
        command: "add",
    },
    Requirement {
        id: "album.remove",
        data_type: "Album",
        command: "remove",
    },
    Requirement {
        id: "album.nest",
        data_type: "Album",
        command: "nest",
    },
    Requirement {
        id: "share.create.gallery",
        data_type: "Picture",
        command: "shareCreate",
    },
    Requirement {
        id: "share.grant",
        data_type: "Picture",
        command: "shareGrant",
    },
    Requirement {
        id: "share.revoke.gallery",
        data_type: "Picture",
        command: "shareRevoke",
    },
    Requirement {
        id: "media.download",
        data_type: "Picture",
        command: "download",
    },
    Requirement {
        id: "media.edit",
        data_type: "Picture",
        command: "edit",
    },
    Requirement {
        id: "media.delete",
        data_type: "Picture",
        command: "delete",
    },
    Requirement {
        id: "media.restore",
        data_type: "Picture",
        command: "restore",
    },
    Requirement {
        id: "media.duplicates",
        data_type: "Picture",
        command: "duplicates",
    },
    Requirement {
        id: "place.list",
        data_type: "Picture",
        command: "placeList",
    },
    Requirement {
        id: "place.assign",
        data_type: "Picture",
        command: "placeAssign",
    },
    Requirement {
        id: "sensitive.mark",
        data_type: "Picture",
        command: "sensitiveMark",
    },
    Requirement {
        id: "sync",
        data_type: "Picture",
        command: "sync",
    },
    Requirement {
        id: "person.detect",
        data_type: "Picture",
        command: "personDetect",
    },
    Requirement {
        id: "person.name",
        data_type: "Picture",
        command: "personName",
    },
    Requirement {
        id: "person.merge",
        data_type: "Picture",
        command: "personMerge",
    },
    Requirement {
        id: "person.link",
        data_type: "Picture",
        command: "personLink",
    },
    Requirement {
        id: "label.suggest",
        data_type: "Picture",
        command: "labelSuggest",
    },
    Requirement {
        id: "media.searchSemantic",
        data_type: "Picture",
        command: "searchSemantic",
    },
    Requirement {
        id: "media.similar",
        data_type: "Picture",
        command: "similar",
    },
    Requirement {
        id: "media.ocr",
        data_type: "Picture",
        command: "ocr",
    },
    Requirement {
        id: "album.query",
        data_type: "Album",
        command: "query",
    },
    Requirement {
        id: "stack.create",
        data_type: "Picture",
        command: "stackCreate",
    },
    Requirement {
        id: "media.favourite",
        data_type: "Picture",
        command: "favourite",
    },
    Requirement {
        id: "media.archive",
        data_type: "Picture",
        command: "archive",
    },
    Requirement {
        id: "media.visibility",
        data_type: "Picture",
        command: "visibility",
    },
    Requirement {
        id: "comment.add",
        data_type: "Picture",
        command: "commentAdd",
    },
    Requirement {
        id: "rating.set",
        data_type: "Picture",
        command: "ratingSet",
    },
    Requirement {
        id: "media.batch",
        data_type: "Picture",
        command: "batch",
    },
    Requirement {
        id: "metadata.export",
        data_type: "Picture",
        command: "metadataExport",
    },
    Requirement {
        id: "place.resolve",
        data_type: "Picture",
        command: "placeResolve",
    },
    Requirement {
        id: "album.suggest",
        data_type: "Album",
        command: "suggest",
    },
    Requirement {
        id: "media.story",
        data_type: "Picture",
        command: "story",
    },
    Requirement {
        id: "media.watermark",
        data_type: "Picture",
        command: "watermark",
    },
    Requirement {
        id: "video.transcode",
        data_type: "Picture",
        command: "transcode",
    },
    Requirement {
        id: "album.sort",
        data_type: "Album",
        command: "sort",
    },
    Requirement {
        id: "media.merge",
        data_type: "Picture",
        command: "merge",
    },
];
