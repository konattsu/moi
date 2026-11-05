pub(crate) struct ExecutionEnv {
    path: std::ffi::OsString,
}

impl ExecutionEnv {
    pub(crate) fn new() -> Self {
        Self {
            path: std::env::var_os("PATH").unwrap_or_default(),
        }
    }
    pub(crate) fn prepend(
        &mut self,
        paths: &[String],
    ) -> Result<(), crate::error::MoiError> {
        let mut values = paths
            .iter()
            .map(|p| crate::path::expand(p))
            .collect::<Result<Vec<_>, _>>()?;
        values.extend(std::env::split_paths(&self.path));
        let mut seen = std::collections::BTreeSet::new();
        values.retain(|v| seen.insert(v.clone()));
        self.path = std::env::join_paths(values)
            .map_err(|e| crate::error::MoiError::config(e.to_string()))?;
        Ok(())
    }
    pub(crate) fn apply_to(&self, command: &mut std::process::Command) {
        command.env("PATH", &self.path);
        sanitize(command);
    }
    pub(crate) fn export(
        &mut self,
        script: &str,
        cwd: &std::path::Path,
        reporter: &mut crate::reporter::Reporter,
    ) -> Result<(), crate::error::MoiError> {
        let output = crate::exec::shell::capture(script, cwd, self, true)?;
        reporter.child_output(&output.stderr);
        if !output.status.success() {
            return Err(crate::error::MoiError::config(format!(
                "env.path_export: exit {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        let path = parse_export(&output.stdout)?;
        self.path = path;
        Ok(())
    }
}

pub(crate) fn sanitize(command: &mut std::process::Command) {
    command
        .env_remove("BASH_ENV")
        .env_remove("SHELLOPTS")
        .env_remove("BASHOPTS")
        .env_remove("ENV");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("BASH_FUNC_") {
            command.env_remove(key);
        }
    }
}

fn parse_export(bytes: &[u8]) -> Result<std::ffi::OsString, crate::error::MoiError> {
    let invalid = || {
        crate::error::MoiError::config(
            "env.path_export: expected nonempty UTF-8 PATH followed by exactly one NUL; elements must be absolute and contain no control characters",
        )
    };
    let value = bytes.strip_suffix(&[0]).ok_or_else(invalid)?;
    let value = std::str::from_utf8(value).map_err(|_| invalid())?;
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(invalid());
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut paths = Vec::new();
    for part in value.split(':') {
        if !part.starts_with('/') {
            return Err(invalid());
        }
        if seen.insert(part) {
            paths.push(part);
        }
    }
    Ok(paths.join(":").into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_protocol_without_lossy_decoding() {
        for bytes in [
            b"".as_slice(),
            b"/bin",
            b"/bin\0\0",
            b"/bin:\0",
            b"bin\0",
            b"/b\xff\0",
            b"/bin\n\0",
        ] {
            assert!(parse_export(bytes).is_err(), "{bytes:?}");
        }
        assert_eq!(
            parse_export(b"/bin:/usr/bin:/bin\0").unwrap(),
            "/bin:/usr/bin"
        );
    }
}
