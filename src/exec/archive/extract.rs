pub(super) fn unpack(
    source: &std::path::Path,
    format: crate::model::Format,
    dst: &std::path::Path,
) -> Result<(), crate::error::MoiError> {
    std::fs::create_dir(dst).map_err(|e| crate::error::MoiError::io(dst, e))?;
    let file = std::fs::File::open(source)
        .map_err(|e| crate::error::MoiError::io(source, e))?;
    let mut extraction = Extraction {
        root: dst,
        seen: std::collections::BTreeSet::new(),
        directories: Vec::new(),
        links: Vec::new(),
    };
    match format {
        crate::model::Format::TarGz => {
            extraction.tar(flate2::read::GzDecoder::new(file))?
        }
        crate::model::Format::TarXz => {
            extraction.tar(xz2::read::XzDecoder::new(file))?
        }
        crate::model::Format::Zip => extraction.zip(file)?,
    }
    extraction.finish()
}
fn error(e: impl std::fmt::Display) -> crate::error::MoiError {
    crate::error::MoiError::config(format!("invalid archive: {e}"))
}
fn relative(
    path: &std::path::Path,
) -> Result<std::path::PathBuf, crate::error::MoiError> {
    if path.is_absolute()
        || path.to_str().is_none()
        || path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(error(format!("unsafe path {}", path.display())));
    }
    Ok(crate::path::normalize(path))
}
fn link_target(
    path: &std::path::Path,
    target: &std::path::Path,
) -> Result<(), crate::error::MoiError> {
    if target.is_absolute()
        || target.as_os_str().is_empty()
        || target.to_str().is_none()
    {
        return Err(error("link target must be relative UTF-8"));
    }
    let mut depth = path
        .parent()
        .map(|p| {
            p.components()
                .filter(|c| matches!(c, std::path::Component::Normal(_)))
                .count()
        })
        .unwrap_or(0);
    for component in target.components() {
        match component {
            std::path::Component::ParentDir => {
                if depth == 0 {
                    return Err(error("link escapes archive root"));
                }
                depth -= 1;
            }
            std::path::Component::Normal(_) => depth += 1,
            std::path::Component::CurDir => {}
            _ => return Err(error("unsafe link")),
        }
    }
    Ok(())
}
struct Extraction<'a> {
    root: &'a std::path::Path,
    seen: std::collections::BTreeSet<std::path::PathBuf>,
    directories: Vec<(std::path::PathBuf, u32)>,
    links: Vec<(std::path::PathBuf, std::path::PathBuf, bool)>,
}
impl Extraction<'_> {
    fn entry(
        &mut self,
        path: &std::path::Path,
        directory: bool,
    ) -> Result<std::path::PathBuf, crate::error::MoiError> {
        let relative = relative(path)?;
        let dst = self.root.join(&relative);
        if !self.seen.insert(relative) {
            return Err(error("duplicate archive entry"));
        }
        crate::path::parents(&dst)?;
        if directory {
            if crate::path::metadata(&dst)?.is_some_and(|m| !m.is_dir()) {
                return Err(error("directory conflicts with file"));
            }
            std::fs::create_dir_all(&dst)
                .map_err(|e| crate::error::MoiError::io(&dst, e))?;
        } else {
            if crate::path::metadata(&dst)?.is_some() {
                return Err(error("file conflicts with existing entry"));
            }
            crate::exec::standard::ensure_parent(&dst)?;
        }
        Ok(dst)
    }
    fn file(
        &mut self,
        path: &std::path::Path,
        mode: u32,
        reader: &mut impl std::io::Read,
    ) -> Result<(), crate::error::MoiError> {
        let dst = self.entry(path, false)?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&dst)
            .map_err(|e| crate::error::MoiError::io(&dst, e))?;
        std::io::copy(reader, &mut file)
            .map_err(|e| crate::error::MoiError::io(&dst, e))?;
        set_mode(&dst, mode)
    }
    fn tar(
        &mut self,
        reader: impl std::io::Read,
    ) -> Result<(), crate::error::MoiError> {
        let mut archive = tar::Archive::new(reader);
        for entry in archive.entries().map_err(error)? {
            let mut entry = entry.map_err(error)?;
            let path = entry.path().map_err(error)?.into_owned();
            let kind = entry.header().entry_type();
            let mode = entry.header().mode().map_err(error)? & 0o7777;
            if kind.is_dir() {
                let dst = self.entry(&path, true)?;
                self.directories.push((dst, mode));
            } else if kind.is_file() {
                self.file(&path, mode, &mut entry)?;
            } else if kind.is_symlink() || kind.is_hard_link() {
                let relative = relative(&path)?;
                let target = entry
                    .link_name()
                    .map_err(error)?
                    .ok_or_else(|| error("missing link target"))?
                    .into_owned();
                if kind.is_symlink() {
                    link_target(&relative, &target)?;
                } else {
                    relative_path_target(&target)?;
                }
                let dst = self.entry(&path, false)?;
                self.links.push((dst, target, kind.is_hard_link()));
            } else {
                return Err(error(format!(
                    "unsupported entry type at {}",
                    path.display()
                )));
            }
        }
        Ok(())
    }
    fn zip(&mut self, file: std::fs::File) -> Result<(), crate::error::MoiError> {
        let mut archive = zip::ZipArchive::new(file).map_err(error)?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(error)?;
            let name = std::str::from_utf8(entry.name_raw())
                .map_err(error)?
                .to_string();
            let path = std::path::Path::new(&name);
            let mode = entry.unix_mode().unwrap_or(if entry.is_dir() {
                0o40755
            } else {
                0o100644
            });
            match mode & 0o170000 {
                0o120000 => {
                    let mut bytes = Vec::new();
                    std::io::Read::read_to_end(&mut entry, &mut bytes)
                        .map_err(error)?;
                    let target = std::path::PathBuf::from(
                        String::from_utf8(bytes).map_err(error)?,
                    );
                    link_target(&relative(path)?, &target)?;
                    let dst = self.entry(path, false)?;
                    self.links.push((dst, target, false));
                }
                0 | 0o040000 | 0o100000 => {
                    if entry.is_dir() {
                        let dst = self.entry(path, true)?;
                        self.directories.push((dst, mode & 0o7777));
                    } else if mode & 0o170000 == 0o040000 {
                        return Err(error("inconsistent zip directory entry"));
                    } else {
                        self.file(path, mode & 0o7777, &mut entry)?;
                    }
                }
                _ => return Err(error("special zip entry")),
            }
        }
        Ok(())
    }
    fn finish(mut self) -> Result<(), crate::error::MoiError> {
        // Resolve hardlinks before introducing any symlinks into the extraction tree.
        let mut hardlinks: Vec<_> = self
            .links
            .iter()
            .filter(|(_, _, hard)| *hard)
            .cloned()
            .collect();
        while !hardlinks.is_empty() {
            let before = hardlinks.len();
            let mut pending = Vec::new();
            for (dst, target, _) in hardlinks {
                let src = self.root.join(relative(&target)?);
                if let Some(meta) = crate::path::metadata(&src)? {
                    if !meta.is_file() {
                        return Err(error("hardlink target is not a regular file"));
                    }
                    crate::path::parents(&dst)?;
                    std::fs::copy(&src, &dst)
                        .map_err(|e| crate::error::MoiError::io(&dst, e))?;
                } else {
                    pending.push((dst, target, true));
                }
            }
            if pending.len() == before {
                return Err(error("missing or cyclic hardlink target"));
            }
            hardlinks = pending;
        }
        for (dst, target, hard) in &self.links {
            if !hard {
                crate::path::parents(dst)?;
                std::os::unix::fs::symlink(target, dst)
                    .map_err(|e| crate::error::MoiError::io(dst, e))?;
            }
        }
        // Check chains, including a link that traverses another internal symlink.
        validate_links(self.root, self.root)?;
        self.directories
            .sort_by_key(|(p, _)| std::cmp::Reverse(p.components().count()));
        for (path, mode) in self.directories {
            set_mode(&path, mode)?;
        }
        Ok(())
    }
}
fn relative_path_target(path: &std::path::Path) -> Result<(), crate::error::MoiError> {
    relative(path).map(|_| ())
}
fn set_mode(path: &std::path::Path, mode: u32) -> Result<(), crate::error::MoiError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|e| crate::error::MoiError::io(path, e))
}
fn validate_links(
    root: &std::path::Path,
    path: &std::path::Path,
) -> Result<(), crate::error::MoiError> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| crate::error::MoiError::io(path, e))?;
    if meta.file_type().is_symlink() {
        let target = std::fs::read_link(path)
            .map_err(|e| crate::error::MoiError::io(path, e))?;
        link_target(path.strip_prefix(root).expect("descendant"), &target)?;
        // canonicalize resolves complete chains. Missing referents are allowed only
        // when every lexical hop remains inside the selected root.
        resolve_link(root, path)?;
    } else if meta.is_dir() {
        for entry in
            std::fs::read_dir(path).map_err(|e| crate::error::MoiError::io(path, e))?
        {
            validate_links(
                root,
                &entry
                    .map_err(|e| crate::error::MoiError::io(path, e))?
                    .path(),
            )?;
        }
    } else if !meta.is_file() {
        return Err(error("special file"));
    }
    Ok(())
}
fn resolve_link(
    root: &std::path::Path,
    path: &std::path::Path,
) -> Result<(), crate::error::MoiError> {
    // Resolve each symlink before processing subsequent `..` components. Lexically
    // normalizing the whole target first would miss escapes through link chains.
    let mut pending: std::collections::VecDeque<std::ffi::OsString> = path
        .strip_prefix(root)
        .expect("contained")
        .components()
        .map(|c| c.as_os_str().to_owned())
        .collect();
    let mut cursor = root.to_path_buf();
    let mut hops = 0;
    while let Some(component) = pending.pop_front() {
        if component == "." {
            continue;
        }
        if component == ".." {
            if cursor == root {
                return Err(error("symlink chain escapes root"));
            }
            cursor.pop();
            continue;
        }
        cursor.push(&component);
        if crate::path::metadata(&cursor)?.is_some_and(|m| m.file_type().is_symlink()) {
            hops += 1;
            if hops > 40 {
                return Err(error("cyclic or excessive symlink chain"));
            }
            let target = std::fs::read_link(&cursor)
                .map_err(|e| crate::error::MoiError::io(&cursor, e))?;
            if target.is_absolute() {
                return Err(error("absolute symlink target"));
            }
            cursor.pop();
            for part in target.components().rev() {
                pending.push_front(part.as_os_str().to_owned());
            }
        }
    }
    Ok(())
}

pub(super) fn copy_tree(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> Result<(), crate::error::MoiError> {
    validate_links(source, source)?;
    fn copy(
        source: &std::path::Path,
        dst: &std::path::Path,
    ) -> Result<(), crate::error::MoiError> {
        let meta = std::fs::symlink_metadata(source)
            .map_err(|e| crate::error::MoiError::io(source, e))?;
        if meta.is_dir() {
            std::fs::create_dir(dst).map_err(|e| crate::error::MoiError::io(dst, e))?;
            for entry in std::fs::read_dir(source)
                .map_err(|e| crate::error::MoiError::io(source, e))?
            {
                let entry = entry.map_err(|e| crate::error::MoiError::io(source, e))?;
                copy(&entry.path(), &dst.join(entry.file_name()))?;
            }
            std::fs::set_permissions(dst, meta.permissions())
                .map_err(|e| crate::error::MoiError::io(dst, e))?;
        } else if meta.is_file() {
            std::fs::copy(source, dst)
                .map_err(|e| crate::error::MoiError::io(dst, e))?;
        } else if meta.file_type().is_symlink() {
            std::os::unix::fs::symlink(
                std::fs::read_link(source)
                    .map_err(|e| crate::error::MoiError::io(source, e))?,
                dst,
            )
            .map_err(|e| crate::error::MoiError::io(dst, e))?;
        } else {
            return Err(error("special file"));
        }
        Ok(())
    }
    copy(source, destination)
}
