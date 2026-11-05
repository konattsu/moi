#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Distro {
    Debian,
    Arch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
pub(crate) enum Arch {
    #[serde(rename = "x86_64")]
    X86_64,
    #[serde(rename = "aarch64")]
    Aarch64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Platform {
    pub(crate) distro: Distro,
    pub(crate) arch: Arch,
    pub(crate) wsl: bool,
}

pub(crate) fn detect() -> Result<Platform, crate::error::MoiError> {
    if std::env::consts::OS != "linux" {
        return Err(crate::error::MoiError::config("only Linux is supported"));
    }
    let os = std::fs::read_to_string("/etc/os-release")
        .map_err(|e| crate::error::MoiError::io("/etc/os-release", e))?;
    let arch = match std::env::consts::ARCH {
        "x86_64" => Arch::X86_64,
        "aarch64" => Arch::Aarch64,
        other => {
            return Err(crate::error::MoiError::config(format!(
                "unsupported architecture: {other}"
            )));
        }
    };
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map_err(|e| crate::error::MoiError::io("/proc/sys/kernel/osrelease", e))?;
    Ok(Platform {
        distro: distro(&os)?,
        arch,
        wsl: release.to_ascii_lowercase().contains("microsoft"),
    })
}

fn distro(text: &str) -> Result<Distro, crate::error::MoiError> {
    let values: std::collections::BTreeMap<_, _> =
        text.lines().filter_map(|l| l.split_once('=')).collect();
    for key in ["ID", "ID_LIKE"] {
        if let Some(value) = values.get(key) {
            for id in value.trim_matches(['\'', '"']).split_whitespace() {
                match id {
                    "debian" | "ubuntu" => return Ok(Distro::Debian),
                    "arch" | "archlinux" => return Ok(Distro::Arch),
                    _ => {}
                }
            }
        }
    }
    Err(crate::error::MoiError::config(
        "unsupported distro in /etc/os-release",
    ))
}
