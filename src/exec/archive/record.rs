#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    schema: u32,
    target: std::path::PathBuf,
    dst: std::path::PathBuf,
    relative: Option<std::path::PathBuf>,
    definition: Definition,
    entries: Option<Vec<Entry>>,
    required: Vec<Required>,
}

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Definition {
    policy: crate::model::Policy,
    version: String,
    sha256: String,
    format: crate::model::Format,
    root: std::path::PathBuf,
    include: Option<Vec<std::path::PathBuf>>,
}

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: std::path::PathBuf,
    value: EntryValue,
}

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
enum EntryValue {
    File { sha256: String, mode: u32 },
    Directory { mode: u32 },
    Symlink { target: std::path::PathBuf },
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Required {
    path: std::path::PathBuf,
    kind: Kind,
    executable: u32,
    referent: Option<(Kind, u32)>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    File,
    Directory,
    Symlink,
}

fn definition(a: &crate::model::Archive) -> Result<Definition, crate::error::MoiError> {
    Ok(Definition {
        policy: a.policy,
        version: a.version.clone(),
        sha256: a.sha256.to_ascii_lowercase(),
        format: a.format,
        root: crate::path::relative(&a.root).map_err(crate::error::MoiError::config)?,
        include: a.selected_paths()?,
    })
}

pub(super) fn directory() -> Result<std::path::PathBuf, crate::error::MoiError> {
    Ok(crate::path::home_dir()?.join(".local/state/moi/archives"))
}
pub(super) fn path_for(
    state: &std::path::Path,
    target: &std::path::Path,
) -> std::path::PathBuf {
    state.join(format!(
        "{}.json",
        hash(target.as_os_str().as_encoded_bytes())
    ))
}
fn hash(bytes: &[u8]) -> String {
    use sha2::Digest;
    hex(&sha2::Sha256::digest(bytes))
}
pub(super) fn digest(path: &std::path::Path) -> Result<String, crate::error::MoiError> {
    use sha2::Digest;
    use std::io::Read;
    let mut file =
        std::fs::File::open(path).map_err(|e| crate::error::MoiError::io(path, e))?;
    let mut digest = sha2::Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|e| crate::error::MoiError::io(path, e))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(hex(&digest.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(result, "{byte:02x}").expect("writing to a String cannot fail");
    }
    result
}

fn load(
    target: &std::path::Path,
    dst: &std::path::Path,
    relative: Option<&std::path::Path>,
    state: &std::path::Path,
) -> Result<Option<Record>, crate::error::MoiError> {
    if target.to_str().is_none() {
        return Err(crate::error::MoiError::config(
            "archive target must be UTF-8",
        ));
    }
    let path = path_for(state, target);
    crate::path::parents(&path)?;
    let Some(meta) = crate::path::metadata(&path)? else {
        return Ok(None);
    };
    if !meta.is_file() {
        return Err(crate::error::MoiError::config(format!(
            "{}: record must be a regular file",
            path.display()
        )));
    }
    let bytes =
        std::fs::read(&path).map_err(|e| crate::error::MoiError::io(&path, e))?;
    let rec: Record = serde_json::from_slice(&bytes).map_err(|e| {
        crate::error::MoiError::config(format!(
            "{}: invalid archive record: {e}",
            path.display()
        ))
    })?;
    if rec.schema != 1
        || rec.target != target
        || rec.dst != dst
        || rec.relative.as_deref() != relative
    {
        return Err(crate::error::MoiError::config(format!(
            "{}: unsupported record version or inconsistent target",
            path.display()
        )));
    }
    rec.validate()
        .map_err(|e| e.context(format!("invalid archive record {}", path.display())))?;
    Ok(Some(rec))
}
impl Record {
    fn validate(&self) -> Result<(), crate::error::MoiError> {
        let invalid = || crate::error::MoiError::config("inconsistent archive record");
        if self.definition.version.is_empty()
            || self.definition.sha256.len() != 64
            || !self
                .definition
                .sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err(invalid());
        }
        fn relative(p: &std::path::Path) -> bool {
            p.to_str()
                .and_then(|s| crate::path::relative(s).ok())
                .is_some_and(|v| v == p)
        }
        if !relative(&self.definition.root) {
            return Err(invalid());
        }
        if let Some(include) = &self.definition.include {
            if include.is_empty()
                || include
                    .iter()
                    .any(|p| !relative(p) || p == std::path::Path::new("."))
                || !include.windows(2).all(|w| w[0] < w[1])
            {
                return Err(invalid());
            }
            if !self.relative.as_ref().is_some_and(|p| include.contains(p)) {
                return Err(invalid());
            }
        } else if self.relative.is_some() {
            return Err(invalid());
        }
        match (self.definition.policy, &self.entries) {
            (crate::model::Policy::Pinned, Some(entries)) => {
                if entries.is_empty()
                    || !self.required.is_empty()
                    || entries[0].path != std::path::Path::new(".")
                {
                    return Err(invalid());
                }
                if !entries.windows(2).all(|w| w[0].path < w[1].path) {
                    return Err(invalid());
                }
                for entry in entries {
                    if !relative(&entry.path) {
                        return Err(invalid());
                    }
                    match &entry.value {
                        EntryValue::File { sha256, mode } => {
                            if sha256.len() != 64
                                || !sha256.bytes().all(|b| b.is_ascii_hexdigit())
                                || *mode > 0o7777
                            {
                                return Err(invalid());
                            }
                        }
                        EntryValue::Directory { mode } => {
                            if *mode > 0o7777 {
                                return Err(invalid());
                            }
                        }
                        EntryValue::Symlink { target } => {
                            if target.is_absolute() {
                                return Err(invalid());
                            }
                        }
                    }
                }
                if self.relative.is_some()
                    && (entries.len() != 1
                        || !matches!(entries[0].value, EntryValue::File { .. }))
                {
                    return Err(invalid());
                }
            }
            (crate::model::Policy::Preserve, None) => {
                let mut paths = std::collections::BTreeSet::new();
                for required in &self.required {
                    if !relative(&required.path)
                        || !paths.insert(&required.path)
                        || required.executable & !0o111 != 0
                        || required
                            .referent
                            .is_some_and(|(_, mode)| mode & !0o111 != 0)
                        || (required.kind == Kind::Symlink)
                            != required.referent.is_some()
                    {
                        return Err(invalid());
                    }
                    if let Some(relative) = &self.relative
                        && *relative != required.path
                    {
                        return Err(invalid());
                    }
                }
            }
            _ => return Err(invalid()),
        }
        Ok(())
    }
}

pub(super) fn satisfied(
    a: &crate::model::Archive,
    dst: &std::path::Path,
    target: &std::path::Path,
    relative: Option<&std::path::Path>,
    state: &std::path::Path,
) -> Result<bool, crate::error::MoiError> {
    let rec = load(target, dst, relative, state)?;
    let existing = crate::path::metadata(target)?;
    let Some(rec) = rec else {
        if existing.is_some() {
            return Err(crate::error::MoiError::config(format!(
                "{}: existing archive target has no management record",
                target.display()
            )));
        }
        return Ok(false);
    };
    if rec.definition != definition(a)? {
        return Ok(false);
    }
    if a.policy == crate::model::Policy::Pinned {
        if existing.is_none() {
            return Ok(false);
        }
        return Ok(rec.entries.as_ref() == Some(&snapshot(target)?));
    }
    if relative.is_none() && existing.is_none() {
        return Ok(false);
    }
    for path in a.required_paths.as_ref().expect("validated preserve") {
        let path =
            crate::path::relative(path).map_err(crate::error::MoiError::config)?;
        if relative.is_some_and(|p| p != path) {
            continue;
        }
        let Some(required) = rec.required.iter().find(|r| r.path == path) else {
            return Ok(false);
        };
        if !check_required(dst, required)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn validate_required(
    a: &crate::model::Archive,
    root: &std::path::Path,
) -> Result<(), crate::error::MoiError> {
    if let Some(paths) = &a.required_paths {
        for p in paths {
            required(
                root,
                &crate::path::relative(p).map_err(crate::error::MoiError::config)?,
            )?;
        }
    }
    Ok(())
}

fn kind(meta: &std::fs::Metadata) -> Result<Kind, crate::error::MoiError> {
    if meta.is_file() {
        Ok(Kind::File)
    } else if meta.is_dir() {
        Ok(Kind::Directory)
    } else if meta.file_type().is_symlink() {
        Ok(Kind::Symlink)
    } else {
        Err(crate::error::MoiError::config(
            "archive contains a special file",
        ))
    }
}
fn executable(meta: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111
}
fn required(
    root: &std::path::Path,
    relative: &std::path::Path,
) -> Result<Required, crate::error::MoiError> {
    let path = root.join(relative);
    let meta = crate::path::metadata(&path)?.ok_or_else(|| {
        crate::error::MoiError::config(format!(
            "{}: missing required path",
            path.display()
        ))
    })?;
    let canonical_root =
        std::fs::canonicalize(root).map_err(|e| crate::error::MoiError::io(root, e))?;
    let canonical = std::fs::canonicalize(&path)
        .map_err(|e| crate::error::MoiError::io(&path, e))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(crate::error::MoiError::config(
            "required path escapes archive destination",
        ));
    }
    let k = kind(&meta)?;
    let referent = if k == Kind::Symlink {
        let meta = std::fs::metadata(&path)
            .map_err(|e| crate::error::MoiError::io(&path, e))?;
        Some((kind(&meta)?, executable(&meta)))
    } else {
        None
    };
    Ok(Required {
        path: relative.into(),
        kind: k,
        executable: executable(&meta),
        referent,
    })
}
fn check_required(
    root: &std::path::Path,
    expected: &Required,
) -> Result<bool, crate::error::MoiError> {
    let path = root.join(&expected.path);
    if crate::path::metadata(&path)?.is_none() {
        return Ok(false);
    }
    let actual = match required(root, &expected.path) {
        Ok(value) => value,
        Err(crate::error::MoiError::Io { source, .. })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            return Ok(false);
        }
        Err(error) => return Err(error),
    };
    Ok(actual.kind == expected.kind
        && actual.executable & expected.executable == expected.executable
        && match (actual.referent, expected.referent) {
            (None, None) => true,
            (Some((ak, am)), Some((ek, em))) => ak == ek && am & em == em,
            _ => false,
        })
}

pub(super) fn create(
    a: &crate::model::Archive,
    dst: &std::path::Path,
    target: &std::path::Path,
    relative: Option<&std::path::Path>,
    candidate: &std::path::Path,
) -> Result<Record, crate::error::MoiError> {
    let mut required_paths = Vec::new();
    if let Some(paths) = &a.required_paths {
        for path in crate::model::normalized_paths(paths)? {
            if let Some(relative) = relative {
                if path == relative {
                    let meta = std::fs::symlink_metadata(candidate)
                        .map_err(|e| crate::error::MoiError::io(candidate, e))?;
                    required_paths.push(Required {
                        path,
                        kind: kind(&meta)?,
                        executable: executable(&meta),
                        referent: None,
                    });
                }
            } else {
                required_paths.push(required(candidate, &path)?);
            }
        }
    }
    Ok(Record {
        schema: 1,
        target: target.into(),
        dst: dst.into(),
        relative: relative.map(Into::into),
        definition: definition(a)?,
        entries: if a.policy == crate::model::Policy::Pinned {
            Some(snapshot(candidate)?)
        } else {
            None
        },
        required: required_paths,
    })
}
pub(super) fn verify_placed(
    a: &crate::model::Archive,
    target: &std::path::Path,
    rec: &Record,
) -> Result<(), crate::error::MoiError> {
    let valid = if a.policy == crate::model::Policy::Pinned {
        rec.entries.as_ref() == Some(&snapshot(target)?)
    } else {
        let mut valid = true;
        for expected in &rec.required {
            if !check_required(&rec.dst, expected)? {
                valid = false;
            }
        }
        valid
    };
    if !valid {
        return Err(crate::error::MoiError::config(format!(
            "{}: placed archive failed verification",
            target.display()
        )));
    }
    Ok(())
}

fn snapshot(root: &std::path::Path) -> Result<Vec<Entry>, crate::error::MoiError> {
    fn visit(
        root: &std::path::Path,
        path: &std::path::Path,
        out: &mut Vec<Entry>,
    ) -> Result<(), crate::error::MoiError> {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::symlink_metadata(path)
            .map_err(|e| crate::error::MoiError::io(path, e))?;
        let relative = path.strip_prefix(root).expect("descendant");
        let relative = if relative.as_os_str().is_empty() {
            std::path::PathBuf::from(".")
        } else {
            relative.into()
        };
        if relative.to_str().is_none() {
            return Err(crate::error::MoiError::config("archive path is not UTF-8"));
        }
        let value = match kind(&meta)? {
            Kind::File => EntryValue::File {
                sha256: digest(path)?,
                mode: meta.permissions().mode() & 0o7777,
            },
            Kind::Directory => EntryValue::Directory {
                mode: meta.permissions().mode() & 0o7777,
            },
            Kind::Symlink => EntryValue::Symlink {
                target: std::fs::read_link(path)
                    .map_err(|e| crate::error::MoiError::io(path, e))?,
            },
        };
        out.push(Entry {
            path: relative,
            value,
        });
        if meta.is_dir() {
            for entry in std::fs::read_dir(path)
                .map_err(|e| crate::error::MoiError::io(path, e))?
            {
                visit(
                    root,
                    &entry
                        .map_err(|e| crate::error::MoiError::io(path, e))?
                        .path(),
                    out,
                )?;
            }
        }
        Ok(())
    }
    let mut entries = Vec::new();
    visit(root, root, &mut entries)?;
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

pub(super) fn save(
    rec: &Record,
    state: &std::path::Path,
) -> Result<(), crate::error::MoiError> {
    use std::io::Write;
    let path = path_for(state, &rec.target);
    crate::path::parents(&path)?;
    if crate::path::metadata(&path)?.is_some_and(|m| !m.is_file()) {
        return Err(crate::error::MoiError::config(
            "record destination is not a regular file",
        ));
    }
    let mut file = tempfile::NamedTempFile::new_in(state)
        .map_err(|e| crate::error::MoiError::io(state, e))?;
    serde_json::to_writer(&mut file, rec)
        .map_err(|e| crate::error::MoiError::config(e.to_string()))?;
    file.write_all(b"\n")
        .map_err(|e| crate::error::MoiError::io(&path, e))?;
    file.as_file()
        .sync_all()
        .map_err(|e| crate::error::MoiError::io(&path, e))?;
    file.persist(&path)
        .map_err(|e| crate::error::MoiError::io(&path, e.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn sha256_matches_known_vectors_for_memory_and_streamed_files() {
        let file = tempfile::NamedTempFile::new().unwrap();
        for (bytes, expected) in [
            (
                Vec::new(),
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                b"abc".to_vec(),
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                vec![b'a'; 1_000_000],
                "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
            ),
        ] {
            std::fs::write(file.path(), &bytes).unwrap();
            assert_eq!(super::hash(&bytes), expected);
            assert_eq!(super::digest(file.path()).unwrap(), expected);
        }
    }
}
