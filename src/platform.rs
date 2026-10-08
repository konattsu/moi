#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Platform {
    Debian,
    Arch,
}

impl Platform {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Platform::Debian => "debian",
            Platform::Arch => "arch",
        }
    }
}

pub(crate) fn detect() -> std::result::Result<Platform, crate::error::MoiError> {
    let values = read_os_release(std::path::Path::new("/etc/os-release"))?;
    let mut ids = Vec::new();
    if let Some(id) = values.get("ID") {
        ids.push(id.to_lowercase());
    }
    if let Some(id_like) = values.get("ID_LIKE") {
        ids.extend(id_like.split_whitespace().map(str::to_lowercase));
    }
    if ids.iter().any(|item| item == "debian" || item == "ubuntu") {
        return Ok(Platform::Debian);
    }
    if ids.iter().any(|item| item == "arch" || item == "archlinux") {
        return Ok(Platform::Arch);
    }
    Err(crate::error::MoiError::config(
        "could not detect supported platform from /etc/os-release",
    ))
}

fn read_os_release(
    path: &std::path::Path,
) -> std::result::Result<
    std::collections::BTreeMap<String, String>,
    crate::error::MoiError,
> {
    let mut values = std::collections::BTreeMap::new();
    if !path.is_file() {
        return Ok(values);
    }
    let content = std::fs::read_to_string(path)
        .map_err(|source| crate::error::MoiError::io(path, source))?;
    for line in content.lines() {
        if line.is_empty() || line.starts_with('#') || !line.contains('=') {
            continue;
        }
        let mut parts = line.splitn(2, '=');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            values.insert(key.to_string(), value.trim().trim_matches('"').to_string());
        }
    }
    Ok(values)
}
