#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct When {
    pub(crate) distro: Option<Vec<crate::platform::Distro>>,
    pub(crate) arch: Option<Vec<crate::platform::Arch>>,
    pub(crate) wsl: Option<bool>,
}

impl When {
    fn validate(&self) -> Result<(), String> {
        fn axis<T: Ord>(values: &Option<Vec<T>>) -> Result<(), String> {
            if let Some(values) = values
                && (values.is_empty()
                    || values
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        != values.len())
            {
                return Err("when arrays must be nonempty and unique".into());
            }
            Ok(())
        }
        axis(&self.distro)?;
        axis(&self.arch)
    }
    pub(crate) fn matches(&self, platform: crate::platform::Platform) -> bool {
        self.distro
            .as_ref()
            .is_none_or(|v| v.contains(&platform.distro))
            && self
                .arch
                .as_ref()
                .is_none_or(|v| v.contains(&platform.arch))
            && self.wsl.is_none_or(|v| v == platform.wsl)
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Operation {
    Directory {
        path: String,
        mode: Option<String>,
        #[serde(default)]
        when: When,
    },
    File {
        src: String,
        dst: String,
        mode: Option<String>,
        #[serde(default)]
        when: When,
    },
    Symlink {
        target: String,
        dst: String,
        #[serde(default)]
        when: When,
    },
    Block {
        src: String,
        dst: String,
        marker: String,
        #[serde(default)]
        when: When,
    },
    Archive(Archive),
    Shell {
        run: String,
        check: Option<String>,
        #[serde(default)]
        when: When,
    },
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Archive {
    pub(crate) dst: String,
    pub(crate) policy: Policy,
    pub(crate) version: String,
    pub(crate) url: String,
    pub(crate) sha256: String,
    pub(crate) format: Format,
    pub(crate) root: String,
    pub(crate) include: Option<Vec<String>>,
    pub(crate) required_paths: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) when: When,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Policy {
    Pinned,
    Preserve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub(crate) enum Format {
    #[serde(rename = "tar.gz")]
    TarGz,
    #[serde(rename = "tar.xz")]
    TarXz,
    #[serde(rename = "zip")]
    Zip,
}

pub(crate) fn mode(value: &Option<String>) -> Result<Option<u32>, String> {
    value
        .as_ref()
        .map(|v| {
            if !(3..=4).contains(&v.len())
                || !v.bytes().all(|b| (b'0'..=b'7').contains(&b))
            {
                return Err("mode must be 3–4 octal digits".into());
            }
            u32::from_str_radix(v, 8).map_err(|e| e.to_string())
        })
        .transpose()
}

impl Operation {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Directory { .. } => "directory",
            Self::File { .. } => "file",
            Self::Symlink { .. } => "symlink",
            Self::Block { .. } => "block",
            Self::Archive(_) => "archive",
            Self::Shell { .. } => "shell",
        }
    }
    pub(crate) fn when(&self) -> &When {
        match self {
            Self::Directory { when, .. }
            | Self::File { when, .. }
            | Self::Symlink { when, .. }
            | Self::Block { when, .. }
            | Self::Shell { when, .. } => when,
            Self::Archive(a) => &a.when,
        }
    }
    pub(crate) fn dst(&self) -> Option<&str> {
        match self {
            Self::Directory { path, .. } => Some(path),
            Self::File { dst, .. }
            | Self::Block { dst, .. }
            | Self::Symlink { dst, .. } => Some(dst),
            Self::Archive(a) => Some(&a.dst),
            Self::Shell { .. } => None,
        }
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        self.when().validate()?;
        if let Some(dst) = self.dst() {
            crate::path::destination(dst)?;
        }
        match self {
            Self::Directory { mode: m, .. } => {
                mode(m)?;
            }
            Self::File { src, mode: m, .. } => {
                crate::path::relative(src)?;
                mode(m)?;
            }
            Self::Block { src, marker, .. } => {
                crate::path::relative(src)?;
                crate::path::text(marker)?;
                if marker.chars().any(char::is_control)
                    || marker.contains(">>>")
                    || marker.contains("<<<")
                {
                    return Err("invalid block marker".into());
                }
            }
            Self::Symlink { target, .. } => crate::path::destination(target)?,
            Self::Shell { run, check, .. } => {
                crate::path::text(run)?;
                if let Some(check) = check {
                    crate::path::text(check)?;
                }
            }
            Self::Archive(a) => a.validate()?,
        }
        Ok(())
    }
}

impl Archive {
    fn validate(&self) -> Result<(), String> {
        crate::path::text(&self.version)?;
        if self.version == "latest" {
            return Err("archive version must be concrete".into());
        }
        crate::path::https(&self.url)?;
        if self.sha256.len() != 64
            || !self.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("sha256 must contain 64 hex digits".into());
        }
        crate::path::relative(&self.root)?;
        for values in [&self.include, &self.required_paths].into_iter().flatten() {
            if values.is_empty() {
                return Err("archive path arrays must be nonempty".into());
            }
            let mut seen = std::collections::BTreeSet::new();
            for path in values {
                crate::path::relative(path)?;
                if !seen.insert(path) {
                    return Err("duplicate archive path".into());
                }
            }
        }
        if self.include.as_ref().is_some_and(|v| {
            v.iter().any(|p| {
                crate::path::relative(p).is_ok_and(|p| p == std::path::Path::new("."))
            })
        }) {
            return Err("include cannot select root".into());
        }
        match (self.policy, &self.required_paths) {
            (Policy::Pinned, Some(_)) => {
                return Err("pinned forbids required_paths".into());
            }
            (Policy::Preserve, None) => {
                return Err("preserve requires required_paths".into());
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn selected_paths(
        &self,
    ) -> Result<Option<Vec<std::path::PathBuf>>, crate::error::MoiError> {
        self.include
            .as_ref()
            .map(|values| normalized_paths(values))
            .transpose()
    }
    pub(crate) fn validate_selection(&self) -> Result<(), crate::error::MoiError> {
        let selected = self.selected_paths()?;
        if let Some(paths) = &selected {
            for (i, a) in paths.iter().enumerate() {
                for b in &paths[i + 1..] {
                    if a.starts_with(b) || b.starts_with(a) {
                        return Err(crate::error::MoiError::config(
                            "include files overlap",
                        ));
                    }
                }
            }
        }
        if let Some(required) = &self.required_paths {
            let required = normalized_paths(required)?;
            if let Some(selected) = selected
                && required.iter().any(|p| !selected.contains(p))
            {
                return Err(crate::error::MoiError::config(
                    "required_paths must be a subset of include",
                ));
            }
        }
        Ok(())
    }
}

pub(crate) fn normalized_paths(
    values: &[String],
) -> Result<Vec<std::path::PathBuf>, crate::error::MoiError> {
    let mut paths = values
        .iter()
        .map(|s| crate::path::relative(s).map_err(crate::error::MoiError::config))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    if paths.windows(2).any(|w| w[0] == w[1]) {
        return Err(crate::error::MoiError::config("duplicate normalized path"));
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    #[test]
    fn conditions_combine_axes_with_and_and_elements_with_or() {
        let operation: super::Operation=toml::from_str("kind='shell'\nrun='true'\nwhen={distro=['debian','arch'],arch=['aarch64'],wsl=true}").unwrap();
        for distro in [
            crate::platform::Distro::Debian,
            crate::platform::Distro::Arch,
        ] {
            for arch in [
                crate::platform::Arch::X86_64,
                crate::platform::Arch::Aarch64,
            ] {
                for wsl in [false, true] {
                    assert_eq!(
                        operation.when().matches(crate::platform::Platform {
                            distro,
                            arch,
                            wsl
                        }),
                        arch == crate::platform::Arch::Aarch64 && wsl
                    );
                }
            }
        }
    }
}
