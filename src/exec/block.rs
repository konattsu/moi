pub(crate) fn plan(
    module: &crate::model::Module,
    block: &crate::model::Block,
) -> std::result::Result<(), crate::error::MoiError> {
    BlockOperation::resolve(module, block)?.describe();
    Ok(())
}

pub(crate) fn apply(
    module: &crate::model::Module,
    block: &crate::model::Block,
    reporter: &mut crate::reporter::Reporter,
) -> std::result::Result<(), crate::error::MoiError> {
    let operation = BlockOperation::resolve(module, block)?;
    reporter.log(operation.description());
    operation.apply()
}

struct BlockOperation {
    src: std::path::PathBuf,
    dst: std::path::PathBuf,
    marker: String,
    platform_label: &'static str,
}

impl BlockOperation {
    fn resolve(
        module: &crate::model::Module,
        block: &crate::model::Block,
    ) -> std::result::Result<Self, crate::error::MoiError> {
        let src = module.path().join(block.src());
        if !src.is_file() {
            return Err(crate::error::MoiError::config(format!(
                "{}: block source not found: {}",
                module.name(),
                src.display()
            )));
        }

        Ok(Self {
            src,
            dst: crate::path::expand_home(block.dst())?,
            marker: block.marker().to_string(),
            platform_label: block.platform().label(),
        })
    }

    fn describe(&self) {
        crate::output!("    {}", self.description());
    }

    fn description(&self) -> String {
        format!(
            "block{} {} -> {} marker={}",
            self.platform_label,
            self.src.display(),
            self.dst.display(),
            self.marker
        )
    }

    fn apply(&self) -> std::result::Result<(), crate::error::MoiError> {
        if let Some(parent) = self.dst.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|source| crate::error::MoiError::io(parent, source))?;
        }
        let content = std::fs::read_to_string(&self.src)
            .map_err(|source| crate::error::MoiError::io(&self.src, source))?;
        let existing = if self.dst.exists() {
            std::fs::read_to_string(&self.dst)
                .map_err(|source| crate::error::MoiError::io(&self.dst, source))?
        } else {
            String::new()
        };
        let updated = replace_or_append(&existing, &self.marker, &content)?;
        std::fs::write(&self.dst, updated)
            .map_err(|source| crate::error::MoiError::io(&self.dst, source))
    }
}

fn replace_or_append(
    existing: &str,
    marker: &str,
    content: &str,
) -> std::result::Result<String, crate::error::MoiError> {
    let start_line = crate::model::Block::start_line(marker);
    let end_line = crate::model::Block::end_line(marker);
    let lines = existing.split_inclusive('\n').collect::<Vec<_>>();
    let starts = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            (line.trim_end_matches('\n') == start_line).then_some(index)
        })
        .collect::<Vec<_>>();
    let ends = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            (line.trim_end_matches('\n') == end_line).then_some(index)
        })
        .collect::<Vec<_>>();
    if starts.len() != ends.len() {
        return Err(crate::error::MoiError::config(format!(
            "marker block is unbalanced: {marker}"
        )));
    }
    if starts.len() > 1 {
        return Err(crate::error::MoiError::config(format!(
            "marker block appears multiple times: {marker}"
        )));
    }
    let replacement = crate::model::Block::text(marker, content);
    if starts.is_empty() {
        let mut prefix = existing.to_string();
        if !prefix.is_empty() && !prefix.ends_with('\n') {
            prefix.push('\n');
        }
        while !prefix.is_empty() && !prefix.ends_with("\n\n\n") {
            prefix.push('\n');
        }
        return Ok(prefix + &replacement);
    }
    let start = starts[0];
    let end = ends[0];
    if start > end {
        return Err(crate::error::MoiError::config(format!(
            "marker block is invalid: {marker}"
        )));
    }
    Ok(lines[..start].concat() + &replacement + &lines[end + 1..].concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_or_append_adds_block() {
        let updated = replace_or_append("hello\n", "moi:test", "body\n").unwrap();

        assert_eq!(
            updated,
            "hello\n\n\n# >>> moi:test >>>\nbody\n# <<< moi:test <<<\n"
        );
    }

    #[test]
    fn test_replace_or_append_replaces_block() {
        let existing = "# >>> moi:test >>>\nold\n# <<< moi:test <<<\n";
        let updated = replace_or_append(existing, "moi:test", "new").unwrap();

        assert_eq!(updated, "# >>> moi:test >>>\nnew\n# <<< moi:test <<<\n");
    }
}
