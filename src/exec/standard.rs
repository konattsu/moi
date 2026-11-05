#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    Changed,
    Unchanged,
    NotApplicable,
    Executed,
}
impl Outcome {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Changed => "changed",
            Self::Unchanged => "unchanged",
            Self::NotApplicable => "not-applicable",
            Self::Executed => "executed",
        }
    }
}

pub(crate) struct Prepared<'a> {
    pub(crate) module: &'a crate::model::Module,
    pub(crate) index: usize,
    pub(crate) operation: &'a crate::model::Operation,
    pub(crate) applicable: bool,
    pub(crate) dst: Option<std::path::PathBuf>,
    // Inputs are frozen before any package or shell operation can change them.
    input: Option<Input>,
}

struct Input {
    bytes: Vec<u8>,
    permissions: std::fs::Permissions,
}

impl<'a> Prepared<'a> {
    pub(crate) fn new(
        module: &'a crate::model::Module,
        index: usize,
        operation: &'a crate::model::Operation,
        platform: crate::platform::Platform,
        repository: &std::path::Path,
    ) -> Result<Self, crate::error::MoiError> {
        let applicable = operation.when().matches(platform);
        let dst = operation.dst().map(crate::path::expand).transpose()?;
        let input = if applicable {
            match operation {
                crate::model::Operation::File { src, .. }
                | crate::model::Operation::Block { src, .. } => {
                    let path = module.path.join(src);
                    let actual = std::fs::canonicalize(&path)
                        .map_err(|e| crate::error::MoiError::io(&path, e))?;
                    let metadata = std::fs::metadata(&actual)
                        .map_err(|e| crate::error::MoiError::io(&actual, e))?;
                    if !actual.starts_with(repository) || !metadata.is_file() {
                        return Err(crate::error::MoiError::config(
                            "src must resolve to a regular static file within the acquired repository",
                        ));
                    }
                    let bytes = std::fs::read(&actual)
                        .map_err(|e| crate::error::MoiError::io(&actual, e))?;
                    if let crate::model::Operation::Block { marker, .. } = operation {
                        let content = std::str::from_utf8(&bytes).map_err(|e| {
                            crate::error::MoiError::config(format!(
                                "block src is not UTF-8: {e}"
                            ))
                        })?;
                        update_block("", marker, content)?;
                    }
                    Some(Input {
                        bytes,
                        permissions: metadata.permissions(),
                    })
                }
                _ => None,
            }
        } else {
            None
        };
        let prepared = Self {
            module,
            index,
            operation,
            applicable,
            dst,
            input,
        };
        if applicable {
            prepared.validate_target()?;
        }
        Ok(prepared)
    }
    pub(crate) fn label(&self) -> String {
        format!(
            "{} operations[{}] {}{}",
            self.module.name,
            self.index,
            self.operation.kind(),
            self.dst
                .as_ref()
                .map(|p| format!(" {}", p.display()))
                .unwrap_or_default()
        )
    }
    pub(crate) fn validate_target(&self) -> Result<(), crate::error::MoiError> {
        let Some(dst) = &self.dst else {
            return Ok(());
        };
        crate::path::parents(dst)?;
        if let crate::model::Operation::Archive(a) = self.operation {
            a.validate_selection()?;
            return crate::exec::archive::validate_targets(a, dst);
        }
        if let Some(meta) = crate::path::metadata(dst)? {
            let valid = match self.operation {
                crate::model::Operation::Directory { .. } => meta.is_dir(),
                crate::model::Operation::Symlink { .. } => {
                    meta.file_type().is_symlink()
                }
                _ => meta.is_file(),
            };
            if !valid {
                return Err(crate::error::MoiError::config(format!(
                    "{}: existing target has a different type",
                    dst.display()
                )));
            }
        }
        Ok(())
    }
    pub(crate) fn satisfied(&self) -> Result<bool, crate::error::MoiError> {
        use crate::model::Operation;
        let Some(dst) = &self.dst else {
            return Ok(false);
        };
        self.validate_target()?;
        let Some(meta) = crate::path::metadata(dst)? else {
            // Archive records must be checked even when the target is absent.
            return if let Operation::Archive(a) = self.operation {
                crate::exec::archive::satisfied(a, dst)
            } else {
                Ok(false)
            };
        };
        match self.operation {
            Operation::Directory { mode, .. } => Ok(mode_matches(&meta, mode)?),
            Operation::File { mode, .. } => Ok(std::fs::read(dst)
                .map_err(|e| crate::error::MoiError::io(dst, e))?
                == self.input.as_ref().expect("file input").bytes
                && mode_matches(&meta, mode)?),
            Operation::Symlink { target, .. } => Ok(std::fs::read_link(dst)
                .map_err(|e| crate::error::MoiError::io(dst, e))?
                == crate::path::expand(target)?),
            Operation::Block { marker, .. } => {
                let old = std::fs::read_to_string(dst)
                    .map_err(|e| crate::error::MoiError::io(dst, e))?;
                Ok(update_block(
                    &old,
                    marker,
                    std::str::from_utf8(
                        &self.input.as_ref().expect("block input").bytes,
                    )
                    .expect("validated UTF-8"),
                )? == old)
            }
            Operation::Archive(a) => crate::exec::archive::satisfied(a, dst),
            Operation::Shell { .. } => Ok(false),
        }
    }
    pub(crate) fn apply(
        &self,
        env: &crate::exec::environment::ExecutionEnv,
        reporter: &mut crate::reporter::Reporter,
    ) -> Result<Outcome, crate::error::MoiError> {
        use crate::model::Operation;
        if !self.applicable {
            return Ok(Outcome::NotApplicable);
        }
        self.validate_target()
            .map_err(|e| e.context("pre-change validation"))?;
        if let Operation::Shell { run, check, .. } = self.operation {
            if let Some(check) = check
                && crate::exec::shell::check(check, &self.module.path, env, reporter)?
            {
                return Ok(Outcome::Unchanged);
            }
            crate::exec::shell::run(run, &self.module.path, env, reporter)
                .map_err(|e| e.context("run"))?;
            if let Some(check) = check
                && !crate::exec::shell::check(check, &self.module.path, env, reporter)
                    .map_err(|e| e.context("post-check"))?
            {
                return Err(crate::error::MoiError::config(
                    "post-check: still unsatisfied after successful run",
                ));
            }
            return Ok(Outcome::Executed);
        }
        if self.satisfied().map_err(|e| e.context("state"))? {
            return Ok(Outcome::Unchanged);
        }
        self.change(env, reporter)
            .map_err(|e| e.context("change"))?;
        if !self.satisfied().map_err(|e| e.context("post-state"))? {
            return Err(crate::error::MoiError::config(
                "post-state: target remains unsatisfied",
            ));
        }
        Ok(Outcome::Changed)
    }

    fn change(
        &self,
        env: &crate::exec::environment::ExecutionEnv,
        reporter: &mut crate::reporter::Reporter,
    ) -> Result<(), crate::error::MoiError> {
        use crate::model::Operation;
        let dst = self.dst.as_ref().expect("standard destination");
        match self.operation {
            Operation::Directory { mode, .. } => {
                std::fs::create_dir_all(dst)
                    .map_err(|e| crate::error::MoiError::io(dst, e))?;
                set_mode(dst, mode)?;
            }
            Operation::File { mode, .. } => {
                ensure_parent(dst)?;
                let permission = if let Some(meta) = crate::path::metadata(dst)? {
                    Some(meta.permissions())
                } else {
                    Some(self.input.as_ref().expect("file input").permissions.clone())
                };
                write_file(
                    dst,
                    &self.input.as_ref().expect("file input").bytes,
                    permission,
                )?;
                set_mode(dst, mode)?;
            }
            Operation::Block { marker, .. } => {
                ensure_parent(dst)?;
                let meta = crate::path::metadata(dst)?;
                let old = if meta.is_some() {
                    std::fs::read_to_string(dst)
                        .map_err(|e| crate::error::MoiError::io(dst, e))?
                } else {
                    String::new()
                };
                let updated = update_block(
                    &old,
                    marker,
                    std::str::from_utf8(
                        &self.input.as_ref().expect("block input").bytes,
                    )
                    .expect("validated UTF-8"),
                )?;
                write_file(dst, updated.as_bytes(), meta.map(|m| m.permissions()))?;
            }
            Operation::Symlink { target, .. } => {
                ensure_parent(dst)?;
                let temp = tempfile::Builder::new()
                    .prefix(".moi-link-")
                    .tempdir_in(dst.parent().expect("absolute parent"))
                    .map_err(|e| crate::error::MoiError::io(dst, e))?;
                let link = temp.path().join("link");
                std::os::unix::fs::symlink(crate::path::expand(target)?, &link)
                    .map_err(|e| crate::error::MoiError::io(&link, e))?;
                std::fs::rename(link, dst)
                    .map_err(|e| crate::error::MoiError::io(dst, e))?;
            }
            Operation::Archive(a) => {
                crate::exec::archive::install(a, dst, env, reporter)?
            }
            Operation::Shell { .. } => unreachable!(),
        }
        Ok(())
    }
}

pub(crate) fn ensure_parent(
    path: &std::path::Path,
) -> Result<(), crate::error::MoiError> {
    crate::path::parents(path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| crate::error::MoiError::io(parent, e))?;
    }
    Ok(())
}

fn write_file(
    path: &std::path::Path,
    bytes: &[u8],
    permissions: Option<std::fs::Permissions>,
) -> Result<(), crate::error::MoiError> {
    use std::io::Write;
    let staging = tempfile::Builder::new()
        .prefix(".moi-file-")
        .tempdir_in(path.parent().expect("absolute parent"))
        .map_err(|e| crate::error::MoiError::io(path, e))?;
    let candidate = staging.path().join("new");
    // File::create_new uses the invoking user's umask for a new block file.
    let mut file = std::fs::File::create_new(&candidate)
        .map_err(|e| crate::error::MoiError::io(path, e))?;
    file.write_all(bytes)
        .map_err(|e| crate::error::MoiError::io(path, e))?;
    if let Some(permissions) = permissions {
        file.set_permissions(permissions)
            .map_err(|e| crate::error::MoiError::io(path, e))?;
    }
    file.sync_all()
        .map_err(|e| crate::error::MoiError::io(path, e))?;
    std::fs::rename(&candidate, path)
        .map_err(|e| crate::error::MoiError::io(path, e))?;
    Ok(())
}

fn mode_matches(
    meta: &std::fs::Metadata,
    mode: &Option<String>,
) -> Result<bool, crate::error::MoiError> {
    use std::os::unix::fs::PermissionsExt;
    Ok(crate::model::parse_mode(mode)
        .map_err(crate::error::MoiError::config)?
        .is_none_or(|m| meta.permissions().mode() & 0o7777 == m))
}
fn set_mode(
    path: &std::path::Path,
    mode: &Option<String>,
) -> Result<(), crate::error::MoiError> {
    use std::os::unix::fs::PermissionsExt;
    if let Some(mode) =
        crate::model::parse_mode(mode).map_err(crate::error::MoiError::config)?
    {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
            .map_err(|e| crate::error::MoiError::io(path, e))?;
    }
    Ok(())
}

fn update_block(
    old: &str,
    marker: &str,
    content: &str,
) -> Result<String, crate::error::MoiError> {
    let start = format!("# >>> {marker} >>>");
    let end = format!("# <<< {marker} <<<");
    if content.lines().any(|line| line == start || line == end) {
        return Err(crate::error::MoiError::config(format!(
            "block source contains its own marker: {marker}"
        )));
    }
    let mut starts = Vec::new();
    let mut ends = Vec::new();
    let mut offset = 0;
    for line in old.split_inclusive('\n') {
        let text = line.trim_end_matches(['\r', '\n']);
        if text == start {
            starts.push((offset, offset + line.len()));
        }
        if text == end {
            ends.push((offset, offset + line.len()));
        }
        offset += line.len();
    }
    if starts.len() != ends.len()
        || starts.len() > 1
        || (!starts.is_empty() && starts[0].0 >= ends[0].0)
    {
        return Err(crate::error::MoiError::config(format!(
            "invalid marker pair: {marker}"
        )));
    }
    let body = if content.is_empty() || content.ends_with('\n') {
        content.to_string()
    } else {
        format!("{content}\n")
    };
    if starts.is_empty() {
        let separator = if old.is_empty() || old.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        return Ok(format!("{old}{separator}{start}\n{body}{end}\n"));
    }
    // Keep marker lines and all bytes outside the managed interval.
    Ok(format!(
        "{}{}{}",
        &old[..starts[0].1],
        body,
        &old[ends[0].0..]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn block_preserves_unmanaged_text_and_rejects_broken_pairs() {
        assert_eq!(
            update_block("before\n# >>> m >>>\nold\n# <<< m <<<\nafter", "m", "new")
                .unwrap(),
            "before\n# >>> m >>>\nnew\n# <<< m <<<\nafter"
        );
        for old in [
            "# >>> m >>>\n",
            "# <<< m <<<\n# >>> m >>>\n",
            "# >>> m >>>\n# >>> m >>>\n# <<< m <<<\n# <<< m <<<\n",
        ] {
            assert!(update_block(old, "m", "body").is_err());
        }
    }
}
