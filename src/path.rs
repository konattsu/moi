pub(crate) fn home_dir() -> Result<std::path::PathBuf, crate::error::MoiError> {
    let path = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| crate::error::MoiError::config("HOME is not set"))?;
    if !path.is_absolute() {
        return Err(crate::error::MoiError::config("HOME must be absolute"));
    }
    Ok(path)
}

pub(crate) fn text(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err("must not be empty".into());
    }
    Ok(())
}

pub(crate) fn name(value: &str) -> Result<(), String> {
    text(value)?;
    if value == "."
        || value == ".."
        || value
            .chars()
            .any(|c| c == '/' || c.is_whitespace() || c.is_control())
    {
        return Err(
            "must be a single path component without whitespace or control characters"
                .into(),
        );
    }
    Ok(())
}

fn path_text(value: &str) -> Result<(), String> {
    text(value)?;
    if value
        .chars()
        .any(|c| c.is_control() || c == '$' || c == '`')
        || value.split('/').any(|p| p == "..")
    {
        return Err("path contains a forbidden character or .. component".into());
    }
    Ok(())
}

pub(crate) fn relative(value: &str) -> Result<std::path::PathBuf, String> {
    path_text(value)?;
    if value.starts_with('/') {
        return Err("path must be relative".into());
    }
    Ok(normalize(std::path::Path::new(value)))
}

pub(crate) fn destination(value: &str) -> Result<(), String> {
    path_text(value)?;
    if !(value.starts_with('/') || value == "~" || value.starts_with("~/")) {
        return Err("path must be absolute or start with ~/".into());
    }
    Ok(())
}

pub(crate) fn expand(
    value: &str,
) -> Result<std::path::PathBuf, crate::error::MoiError> {
    destination(value).map_err(crate::error::MoiError::config)?;
    let path = if value == "~" {
        home_dir()?
    } else if let Some(rest) = value.strip_prefix("~/") {
        home_dir()?.join(rest)
    } else {
        std::path::PathBuf::from(value)
    };
    Ok(normalize(&path))
}

pub(crate) fn normalize(path: &std::path::Path) -> std::path::PathBuf {
    let mut result = std::path::PathBuf::new();
    for part in path.components() {
        if part != std::path::Component::CurDir {
            result.push(part);
        }
    }
    if result.as_os_str().is_empty() {
        result.push(".");
    }
    result
}

pub(crate) fn metadata(
    path: &std::path::Path,
) -> Result<Option<std::fs::Metadata>, crate::error::MoiError> {
    match std::fs::symlink_metadata(path) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(crate::error::MoiError::io(path, error)),
    }
}

pub(crate) fn parents(path: &std::path::Path) -> Result<(), crate::error::MoiError> {
    for parent in path.ancestors().skip(1) {
        if let Some(meta) = metadata(parent)?
            && (!meta.is_dir() || meta.file_type().is_symlink())
        {
            return Err(crate::error::MoiError::config(format!(
                "{}: parent must be a real directory",
                parent.display()
            )));
        }
    }
    Ok(())
}

pub(crate) fn https(value: &str) -> Result<url::Url, String> {
    if value.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("URL contains whitespace or control characters".into());
    }
    let parsed = url::Url::parse(value).map_err(|e| e.to_string())?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() {
        return Err("expected an HTTPS URL with a host".into());
    }
    Ok(parsed)
}
