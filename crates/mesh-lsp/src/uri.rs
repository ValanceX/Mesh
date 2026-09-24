//! `file:` URIs, file system paths, and the workspace-relative keys that
//! `mesh.components` maps to components (outline D6).
//!
//! `lsp-types`' `Uri` has no file path conversion, so this module does it
//! for the one scheme the server understands. A URI of any other scheme
//! (`untitled:`, a virtual document) has no path, and so no key: such a
//! document is checked without a model.

use std::path::{Component, Path, PathBuf};

/// The path a `file:` URI names. `None` for another scheme, a host other
/// than `localhost`, or an escape that isn't valid UTF-8.
pub(crate) fn to_path(uri: &str) -> Option<PathBuf> {
    let scheme = uri.get(..7)?;
    if !scheme.eq_ignore_ascii_case("file://") {
        return None;
    }
    let rest = &uri[7..];
    let slash = rest.find('/')?;
    let (host, path) = rest.split_at(slash);
    if !(host.is_empty() || host.eq_ignore_ascii_case("localhost")) {
        return None;
    }
    let path = path.split(['?', '#']).next()?;
    let path = percent_decode(path)?;
    // `file:///C:/x` names `C:/x`, a Windows drive path.
    let path = match path.as_bytes() {
        [b'/', drive, b':', ..] if drive.is_ascii_alphabetic() => path[1..].to_string(),
        _ => path,
    };
    Some(PathBuf::from(path))
}

/// The `file:` URI of an absolute `path`.
pub(crate) fn from_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    let text = if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text.into_owned()
    };
    let mut uri = String::from("file://");
    if !text.starts_with('/') {
        uri.push('/');
    }
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                uri.push(char::from(byte));
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}

/// `path`'s key relative to `root`: its segments below the root, joined
/// with `/`. `None` if it isn't below the root, or if either path has a
/// `..` segment. Paths compare exactly, segment by segment, with no
/// case folding and no symbolic link resolution.
pub(crate) fn key(root: &Path, path: &Path) -> Option<String> {
    let root = lexical(root, false)?;
    let path = lexical(path, false)?;
    let relative = path.strip_prefix(&root).ok()?;
    let segments: Vec<&str> = relative
        .components()
        .map(|component| match component {
            Component::Normal(segment) => segment.to_str(),
            _ => None,
        })
        .collect::<Option<_>>()?;
    (!segments.is_empty()).then(|| segments.join("/"))
}

/// A configured key, as a user wrote it (`./pages/home.mprx`), in the
/// form [`key`] produces (`pages/home.mprx`). `None` if it can't name a
/// document below the root.
pub(crate) fn normalize_key(written: &str) -> Option<String> {
    let segments: Vec<&str> = written
        .split(['/', '\\'])
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect();
    if segments.is_empty() || segments.contains(&"..") {
        return None;
    }
    Some(segments.join("/"))
}

/// `written`, a path from the configuration, resolved against `root`:
/// relative paths are joined to it, `.` segments dropped and `..`
/// segments applied, so a model may live beside the workspace.
pub(crate) fn resolve(root: &Path, written: &str) -> Option<PathBuf> {
    let written = Path::new(written);
    let joined = if written.is_absolute() {
        written.to_path_buf()
    } else {
        root.join(written)
    };
    lexical(&joined, true)
}

/// `path` with `.` segments removed and, if `apply_parent`, `..`
/// segments applied. Without it, a `..` segment makes the path invalid.
fn lexical(path: &Path, apply_parent: bool) -> Option<PathBuf> {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir if apply_parent => {
                result.pop();
            }
            Component::ParentDir => return None,
            other => result.push(other),
        }
    }
    Some(result)
}

/// Decodes `%XX` escapes. `None` for a malformed escape, or bytes that
/// aren't UTF-8.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text.get(index + 1..index + 3)?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn reads_file_uris() {
        assert_eq!(
            to_path("file:///a/b.mprx"),
            Some(PathBuf::from("/a/b.mprx"))
        );
        assert_eq!(
            to_path("file://localhost/a/b.mprx"),
            Some(PathBuf::from("/a/b.mprx"))
        );
        assert_eq!(
            to_path("FILE:///a%20dir/%C3%A9.mprx"),
            Some(PathBuf::from("/a dir/é.mprx"))
        );
        assert_eq!(
            to_path("file:///C:/x/y.mprx"),
            Some(PathBuf::from("C:/x/y.mprx"))
        );
        assert_eq!(
            to_path("file:///a/b.mprx?x#y"),
            Some(PathBuf::from("/a/b.mprx"))
        );
    }

    #[test]
    fn other_uris_have_no_path() {
        assert_eq!(to_path("untitled:Untitled-1"), None);
        assert_eq!(to_path("file://server/share/a.mprx"), None);
        assert_eq!(to_path("file:///a%2"), None);
        assert_eq!(to_path("file:///a%FF"), None);
        assert_eq!(to_path("file:"), None);
        assert_eq!(to_path(""), None);
    }

    #[test]
    fn writes_file_uris_that_read_back() {
        for path in ["/a/b.mprx", "/a dir/é.mprx", "/x/100%/y#z.mprx"] {
            let uri = from_path(Path::new(path));
            assert_eq!(to_path(&uri), Some(PathBuf::from(path)), "{uri}");
        }
        assert_eq!(from_path(Path::new("/a b")), "file:///a%20b");
    }

    #[test]
    fn keys_are_relative_to_the_root() {
        let root = Path::new("/w");
        assert_eq!(key(root, Path::new("/w/a.mprx")), Some("a.mprx".into()));
        assert_eq!(
            key(root, Path::new("/w/./p/a.mprx")),
            Some("p/a.mprx".into())
        );
        assert_eq!(key(root, Path::new("/other/a.mprx")), None);
        assert_eq!(key(root, Path::new("/w/../w/a.mprx")), None);
        assert_eq!(key(root, Path::new("/w")), None);
    }

    #[test]
    fn configured_keys_are_normalized() {
        assert_eq!(normalize_key("./p//a.mprx"), Some("p/a.mprx".into()));
        assert_eq!(normalize_key("p\\a.mprx"), Some("p/a.mprx".into()));
        assert_eq!(normalize_key("../a.mprx"), None);
        assert_eq!(normalize_key("."), None);
    }

    #[test]
    fn configured_paths_resolve_against_the_root() {
        let root = Path::new("/w/project");
        assert_eq!(
            resolve(root, "components.json"),
            Some(PathBuf::from("/w/project/components.json"))
        );
        assert_eq!(
            resolve(root, "../shared/./components.json"),
            Some(PathBuf::from("/w/shared/components.json"))
        );
        assert_eq!(
            resolve(root, "/abs/components.json"),
            Some(PathBuf::from("/abs/components.json"))
        );
    }
}
