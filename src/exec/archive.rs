mod extract;
mod record;

pub(crate) fn unpack_release(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> Result<(), crate::error::MoiError> {
    extract::unpack(source, crate::model::Format::TarGz, destination)
}

pub(crate) fn checksum(
    source: &std::path::Path,
) -> Result<String, crate::error::MoiError> {
    record::digest(source)
}

pub(crate) fn validate_targets(
    archive: &crate::model::Archive,
    dst: &std::path::Path,
) -> Result<(), crate::error::MoiError> {
    crate::path::parents(dst)?;
    if let Some(meta) = crate::path::metadata(dst)?
        && !meta.is_dir()
    {
        return Err(crate::error::MoiError::config(format!(
            "{}: archive destination must be a real directory",
            dst.display()
        )));
    }
    if let Some(include) = archive.selected_paths()? {
        for relative in include {
            let target = dst.join(relative);
            crate::path::parents(&target)?;
            if let Some(meta) = crate::path::metadata(&target)?
                && !meta.is_file()
            {
                return Err(crate::error::MoiError::config(format!(
                    "{}: archive target must be a regular file",
                    target.display()
                )));
            }
        }
    }
    Ok(())
}

pub(crate) fn satisfied(
    archive: &crate::model::Archive,
    dst: &std::path::Path,
) -> Result<bool, crate::error::MoiError> {
    satisfied_in(archive, dst, &record::directory()?)
}

fn satisfied_in(
    archive: &crate::model::Archive,
    dst: &std::path::Path,
    state: &std::path::Path,
) -> Result<bool, crate::error::MoiError> {
    validate_targets(archive, dst)?;
    let mut satisfied = true;
    for (target, relative) in targets(archive, dst)? {
        // Do not short circuit: a corrupt record or unowned target is always an error.
        if !record::satisfied(archive, dst, &target, relative.as_deref(), state)? {
            satisfied = false;
        }
    }
    Ok(satisfied)
}

fn targets(
    archive: &crate::model::Archive,
    dst: &std::path::Path,
) -> Result<Vec<(std::path::PathBuf, Option<std::path::PathBuf>)>, crate::error::MoiError>
{
    Ok(match archive.selected_paths()? {
        Some(paths) => paths.into_iter().map(|p| (dst.join(&p), Some(p))).collect(),
        None => vec![(dst.to_path_buf(), None)],
    })
}

pub(crate) fn install(
    archive: &crate::model::Archive,
    dst: &std::path::Path,
    env: &crate::exec::environment::ExecutionEnv,
    reporter: &mut crate::reporter::Reporter,
) -> Result<(), crate::error::MoiError> {
    let work = tempfile::Builder::new()
        .prefix("moi-archive-")
        .tempdir()
        .map_err(|e| crate::error::MoiError::io("archive temporary directory", e))?;
    let downloaded = work.path().join("download");
    let mut command = std::process::Command::new("curl");
    command
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--output",
        ])
        .arg(&downloaded)
        .arg("--")
        .arg(&archive.url);
    env.apply_to(&mut command);
    crate::exec::shell::run_command(&mut command, reporter)
        .map_err(|e| e.context("archive download"))?;
    install_downloaded(
        archive,
        dst,
        &downloaded,
        work.path(),
        &record::directory()?,
    )
}

fn install_downloaded(
    archive: &crate::model::Archive,
    dst: &std::path::Path,
    downloaded: &std::path::Path,
    work: &std::path::Path,
    state: &std::path::Path,
) -> Result<(), crate::error::MoiError> {
    if record::digest(downloaded)? != archive.sha256.to_ascii_lowercase() {
        return Err(crate::error::MoiError::config("archive checksum mismatch"));
    }
    let unpacked = work.join("unpacked");
    extract::unpack(downloaded, archive.format, &unpacked)
        .map_err(|e| e.context("archive extraction"))?;
    let root = unpacked.join(
        crate::path::relative(&archive.root).map_err(crate::error::MoiError::config)?,
    );
    crate::path::parents(&root)?;
    if !crate::path::metadata(&root)?.is_some_and(|m| m.is_dir()) {
        return Err(crate::error::MoiError::config(
            "archive root must be a real directory",
        ));
    }
    let selected = targets(archive, dst)?;
    // Validate every candidate and record before moving any existing target.
    for (_, relative) in &selected {
        if let Some(relative) = relative {
            let src = root.join(relative);
            crate::path::parents(&src)?;
            if !crate::path::metadata(&src)?.is_some_and(|m| m.is_file()) {
                return Err(crate::error::MoiError::config(format!(
                    "{}: include must select a regular file",
                    src.display()
                )));
            }
        }
    }
    record::validate_required(archive, &root)?;
    validate_targets(archive, dst)?;
    satisfied_in(archive, dst, state)?;
    let mut replacements = Vec::new();
    for (target, relative) in &selected {
        crate::exec::standard::ensure_parent(target)?;
        let staging = tempfile::Builder::new()
            .prefix(".moi-archive-")
            .tempdir_in(target.parent().expect("absolute parent"))
            .map_err(|e| crate::error::MoiError::io(target, e))?;
        let candidate = staging.path().join("new");
        let source = relative
            .as_ref()
            .map(|p| root.join(p))
            .unwrap_or_else(|| root.clone());
        extract::copy_tree(&source, &candidate)?;
        let rec =
            record::create(archive, dst, target, relative.as_deref(), &candidate)?;
        replacements.push(Replacement {
            target: target.clone(),
            staging,
            record: rec,
            moved: false,
        });
    }
    // All candidates now exist on their respective target filesystems.
    crate::path::parents(&state.join("record"))?;
    std::fs::create_dir_all(state).map_err(|e| crate::error::MoiError::io(state, e))?;
    finish_replacements(archive, dst, replacements, state)
}

fn finish_replacements(
    archive: &crate::model::Archive,
    dst: &std::path::Path,
    mut replacements: Vec<Replacement>,
    state: &std::path::Path,
) -> Result<(), crate::error::MoiError> {
    let result = commit(archive, dst, &mut replacements, state);
    if let Err(error) = result {
        let paths = replacements
            .iter()
            .map(|r| {
                format!(
                    "target={} record={} temporary={} backup={}",
                    r.target.display(),
                    record::path_for(state, &r.target).display(),
                    r.staging.path().display(),
                    r.staging.path().join("old").display()
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        for replacement in replacements {
            if replacement.moved {
                let _ = replacement.staging.keep();
            }
        }
        return Err(error.context(format!("archive placement failed; {paths}")));
    }
    // Disarm all automatic cleanup first. A cleanup error must not drop the
    // remaining TempDirs and silently erase their backups while returning failure.
    let cleanup: Vec<_> = replacements
        .into_iter()
        .map(|r| (r.target, r.staging.keep()))
        .collect();
    for (target, temporary) in cleanup {
        std::fs::remove_dir_all(&temporary).map_err(|e| {
            crate::error::MoiError::io(&temporary, e).context(format!(
                "archive cleanup failed: target={} record={} temporary={} backup={}",
                target.display(),
                record::path_for(state, &target).display(),
                temporary.display(),
                temporary.join("old").display()
            ))
        })?;
    }
    Ok(())
}

struct Replacement {
    target: std::path::PathBuf,
    staging: tempfile::TempDir,
    record: record::Record,
    moved: bool,
}
fn commit(
    archive: &crate::model::Archive,
    dst: &std::path::Path,
    replacements: &mut [Replacement],
    state: &std::path::Path,
) -> Result<(), crate::error::MoiError> {
    for replacement in replacements.iter_mut() {
        validate_targets(archive, dst)?;
        let backup = replacement.staging.path().join("old");
        if crate::path::metadata(&replacement.target)?.is_some() {
            std::fs::rename(&replacement.target, &backup)
                .map_err(|e| crate::error::MoiError::io(&replacement.target, e))?;
        }
        replacement.moved = true;
        std::fs::rename(replacement.staging.path().join("new"), &replacement.target)
            .map_err(|e| crate::error::MoiError::io(&replacement.target, e))?;
    }
    for replacement in replacements.iter() {
        record::verify_placed(archive, &replacement.target, &replacement.record)?;
    }
    for replacement in replacements.iter() {
        record::save(&replacement.record, state)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
