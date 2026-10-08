pub(crate) fn plan(
    module: &crate::model::Module,
    file: &crate::model::File,
) -> std::result::Result<(), crate::error::MoiError> {
    FileOperation::resolve(module, file)?.describe();
    Ok(())
}

pub(crate) fn apply(
    module: &crate::model::Module,
    file: &crate::model::File,
    reporter: &mut crate::reporter::Reporter,
) -> std::result::Result<(), crate::error::MoiError> {
    let operation = FileOperation::resolve(module, file)?;
    reporter.log(operation.description());
    operation.apply()
}

struct FileOperation {
    src: std::path::PathBuf,
    dst: std::path::PathBuf,
    platform_label: &'static str,
    mode: Option<crate::model::Mode>,
}

impl FileOperation {
    fn resolve(
        module: &crate::model::Module,
        file: &crate::model::File,
    ) -> std::result::Result<Self, crate::error::MoiError> {
        let src = module.path().join(file.src());
        if !src.is_file() {
            return Err(crate::error::MoiError::config(format!(
                "{}: file source not found: {}",
                module.name(),
                src.display()
            )));
        }

        Ok(Self {
            src,
            dst: crate::path::expand_home(file.dst())?,
            platform_label: file.platform().label(),
            mode: file.mode(),
        })
    }

    fn describe(&self) {
        crate::output!("    {}", self.description());
    }

    fn description(&self) -> String {
        format!(
            "file{} {} -> {}{}",
            self.platform_label,
            self.src.display(),
            self.dst.display(),
            self.mode
                .map(|mode| format!(" mode={mode}"))
                .unwrap_or_default()
        )
    }

    fn apply(&self) -> std::result::Result<(), crate::error::MoiError> {
        if let Some(parent) = self.dst.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|source| crate::error::MoiError::io(parent, source))?;
        }
        std::fs::copy(&self.src, &self.dst)
            .map_err(|source| crate::error::MoiError::io(&self.dst, source))?;
        if let Some(mode) = self.mode {
            crate::exec::path::set_mode(&self.dst, mode)?;
        }
        Ok(())
    }
}
