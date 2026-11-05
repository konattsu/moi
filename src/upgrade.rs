pub(crate) fn run(
    args: &crate::cli::UpgradeArgs,
    reporter: &mut crate::reporter::Reporter,
) -> std::result::Result<(), crate::error::MoiError> {
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|source| crate::error::MoiError::config(source.to_string()))?;
    let latest = latest_version()?;

    if current > latest {
        reporter.log(format!(
            "moi v{current} is newer than the latest release (v{latest})."
        ));
        return Ok(());
    }
    if current == latest && !args.force {
        reporter.log(format!("moi v{current} is already up to date."));
        return Ok(());
    }

    if current == latest {
        reporter.log(format!("Reinstalling moi v{current}..."));
    } else {
        reporter.log(format!("Upgrading moi v{current} to v{latest}..."));
    }
    install_latest(reporter)?;
    reporter.log(format!("Successfully upgraded moi to v{latest}."));
    Ok(())
}

fn latest_version() -> std::result::Result<semver::Version, crate::error::MoiError> {
    let repository = std::env::var("MOI_GITHUB_REPOSITORY")
        .unwrap_or_else(|_| "konattsu/moi".to_string());
    let url = std::env::var("MOI_LATEST_RELEASE_URL")
        .unwrap_or_else(|_| format!("https://github.com/{repository}/releases/latest"));
    let mut command = std::process::Command::new("curl");
    command.args(["-fsSL", "-o", "/dev/null", "-w", "%{url_effective}", &url]);
    crate::exec::environment::sanitize(&mut command);
    let output = command
        .output()
        .map_err(|source| crate::error::MoiError::io("curl", source))?;
    if !output.status.success() {
        return Err(crate::error::MoiError::config(format!(
            "release lookup failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let effective_url = std::str::from_utf8(&output.stdout)
        .map_err(|source| crate::error::MoiError::config(source.to_string()))?;
    parse_release_version(effective_url)
}

fn parse_release_version(
    effective_url: &str,
) -> std::result::Result<semver::Version, crate::error::MoiError> {
    let tag = effective_url
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|tag| !tag.is_empty() && *tag != "latest")
        .ok_or_else(|| {
            crate::error::MoiError::config(format!(
                "could not determine latest release from {effective_url}"
            ))
        })?;
    semver::Version::parse(tag.strip_prefix('v').unwrap_or(tag)).map_err(|source| {
        crate::error::MoiError::config(format!(
            "invalid release version {tag}: {source}"
        ))
    })
}

fn install_latest(
    reporter: &mut crate::reporter::Reporter,
) -> std::result::Result<(), crate::error::MoiError> {
    let target = target_triple()?;
    let package = format!("moi-{target}");
    let asset = std::env::var("MOI_RELEASE_ASSET")
        .unwrap_or_else(|_| format!("{package}.tar.gz"));
    crate::path::name(&asset).map_err(crate::error::MoiError::config)?;
    let repository = std::env::var("MOI_GITHUB_REPOSITORY")
        .unwrap_or_else(|_| "konattsu/moi".to_string());
    let release_base = std::env::var("MOI_RELEASE_BASE").unwrap_or_else(|_| {
        format!("https://github.com/{repository}/releases/latest/download")
    });
    let url = format!("{}/{asset}", release_base.trim_end_matches('/'));
    let tempdir = tempfile::Builder::new()
        .prefix("moi-upgrade-")
        .tempdir()
        .map_err(|source| crate::error::MoiError::io(std::env::temp_dir(), source))?;
    let archive = tempdir.path().join(&asset);
    let checksum = tempdir.path().join(format!("{asset}.sha256"));

    curl_download(&url, &archive, reporter)?;
    curl_download(&format!("{url}.sha256"), &checksum, reporter)?;
    let checksum_text = std::fs::read_to_string(&checksum)
        .map_err(|e| crate::error::MoiError::io(&checksum, e))?;
    let expected = release_checksum(&checksum_text, &asset)?;
    if crate::exec::archive::checksum(&archive)? != expected {
        return Err(crate::error::MoiError::config("release checksum mismatch"));
    }
    let unpacked = tempdir.path().join("unpacked");
    crate::exec::archive::unpack_release(&archive, &unpacked)?;
    let binary = unpacked.join(package).join("moi");
    crate::path::parents(&binary)?;
    if !crate::path::metadata(&binary)?.is_some_and(|m| m.is_file()) {
        return Err(crate::error::MoiError::config(
            "release binary must be a regular file",
        ));
    }
    replace_current_executable(&binary)
}

fn curl_download(
    url: &str,
    destination: &std::path::Path,
    reporter: &mut crate::reporter::Reporter,
) -> std::result::Result<(), crate::error::MoiError> {
    crate::exec::shell::run_command(
        std::process::Command::new("curl")
            .args(["-fsSL", url, "-o"])
            .arg(destination),
        reporter,
    )
}

fn release_checksum(text: &str, asset: &str) -> Result<String, crate::error::MoiError> {
    let mut matches = Vec::new();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        if let (Some(digest), Some(filename), None) =
            (fields.next(), fields.next(), fields.next())
            && filename.trim_start_matches('*') == asset
            && digest.len() == 64
            && digest.bytes().all(|b| b.is_ascii_hexdigit())
        {
            matches.push(digest.to_ascii_lowercase());
        }
    }
    if matches.len() != 1 {
        return Err(crate::error::MoiError::config(
            "release checksum must uniquely identify the downloaded asset",
        ));
    }
    Ok(matches.remove(0))
}

fn replace_current_executable(
    source: &std::path::Path,
) -> std::result::Result<(), crate::error::MoiError> {
    let destination = std::env::current_exe()
        .map_err(|source| crate::error::MoiError::io("current executable", source))?;
    let parent = destination.parent().ok_or_else(|| {
        crate::error::MoiError::config(format!(
            "current executable has no parent directory: {}",
            destination.display()
        ))
    })?;
    let mut replacement = tempfile::NamedTempFile::new_in(parent)
        .map_err(|source| crate::error::MoiError::io(parent, source))?;
    let mut downloaded = std::fs::File::open(source)
        .map_err(|error| crate::error::MoiError::io(source, error))?;
    std::io::copy(&mut downloaded, replacement.as_file_mut())
        .map_err(|error| crate::error::MoiError::io(replacement.path(), error))?;

    let permissions = std::fs::metadata(source)
        .map_err(|error| crate::error::MoiError::io(source, error))?
        .permissions();
    replacement
        .as_file()
        .set_permissions(permissions)
        .map_err(|error| crate::error::MoiError::io(replacement.path(), error))?;
    replacement
        .as_file_mut()
        .sync_all()
        .map_err(|error| crate::error::MoiError::io(replacement.path(), error))?;
    replacement
        .persist(&destination)
        .map_err(|error| crate::error::MoiError::io(&destination, error.error))?;
    Ok(())
}

fn target_triple() -> std::result::Result<&'static str, crate::error::MoiError> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Ok("aarch64-unknown-linux-gnu"),
        (os, arch) => Err(crate::error::MoiError::config(format!(
            "unsupported platform: {os}/{arch}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_release_version() {
        let version = super::parse_release_version(
            "https://github.com/konattsu/moi/releases/tag/v1.2.3",
        )
        .unwrap();

        assert_eq!(version, semver::Version::new(1, 2, 3));
    }

    #[test]
    fn test_parse_release_version_without_v_prefix() {
        let version = super::parse_release_version(
            "https://github.com/konattsu/moi/releases/tag/1.2.3/",
        )
        .unwrap();

        assert_eq!(version, semver::Version::new(1, 2, 3));
    }

    #[test]
    fn test_reject_latest_without_redirect() {
        let error = super::parse_release_version(
            "https://github.com/konattsu/moi/releases/latest",
        )
        .unwrap_err();

        assert!(error.to_string().contains("could not determine"));
    }
}

#[cfg(test)]
mod checksum_tests {
    #[test]
    fn release_checksum_identifies_the_exact_asset() {
        let digest = "a".repeat(64);
        assert_eq!(
            super::release_checksum(&format!("{digest}  moi.tar.gz\n"), "moi.tar.gz")
                .unwrap(),
            digest
        );
        for text in [
            format!("{digest}  different.tar.gz"),
            format!("{digest}  moi.tar.gz\n{digest}  moi.tar.gz"),
            "invalid  moi.tar.gz".into(),
        ] {
            assert!(super::release_checksum(&text, "moi.tar.gz").is_err());
        }
    }
}
