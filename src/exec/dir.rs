pub(crate) fn plan(
    dir: &crate::model::Dir,
) -> std::result::Result<(), crate::error::MoiError> {
    DirOperation::resolve(dir)?.describe();
    Ok(())
}

pub(crate) fn apply(
    dir: &crate::model::Dir,
    reporter: &mut crate::reporter::Reporter,
) -> std::result::Result<(), crate::error::MoiError> {
    let operation = DirOperation::resolve(dir)?;
    reporter.log(operation.description());
    operation.apply()
}

struct DirOperation {
    path: std::path::PathBuf,
    platform_label: &'static str,
    mode: Option<crate::model::Mode>,
}

impl DirOperation {
    fn resolve(
        dir: &crate::model::Dir,
    ) -> std::result::Result<Self, crate::error::MoiError> {
        Ok(Self {
            path: crate::path::expand_home(dir.path())?,
            platform_label: dir.platform().label(),
            mode: dir.mode(),
        })
    }

    fn describe(&self) {
        crate::output!("    {}", self.description());
    }

    fn description(&self) -> String {
        format!(
            "dir{} {}{}",
            self.platform_label,
            self.path.display(),
            self.mode
                .map(|mode| format!(" mode={mode}"))
                .unwrap_or_default()
        )
    }

    fn apply(&self) -> std::result::Result<(), crate::error::MoiError> {
        std::fs::create_dir_all(&self.path)
            .map_err(|source| crate::error::MoiError::io(&self.path, source))?;
        if let Some(mode) = self.mode {
            crate::exec::path::set_mode(&self.path, mode)?;
        }
        Ok(())
    }
}
