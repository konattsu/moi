mod operation;
pub(crate) use operation::{mode as parse_mode, normalized_paths};
pub(crate) mod modules;
pub(crate) use operation::{Archive, Format, Operation, Policy};

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Module {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) depends_on: Vec<String>,
    #[serde(default)]
    pub(crate) note: Vec<String>,
    #[serde(default)]
    pub(crate) packages: Packages,
    #[serde(default)]
    pub(crate) env: Env,
    #[serde(default)]
    pub(crate) operations: Vec<Operation>,
    #[serde(skip)]
    pub(crate) path: std::path::PathBuf,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Packages {
    #[serde(default)]
    pub(crate) apt: Vec<String>,
    #[serde(default)]
    pub(crate) pacman: Vec<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Env {
    #[serde(default)]
    pub(crate) path_prepend: Vec<String>,
    pub(crate) path_export: Option<String>,
}

impl Module {
    pub(crate) fn validate(&self) -> Result<(), String> {
        crate::path::name(&self.name)?;
        let mut seen = std::collections::BTreeSet::new();
        for name in &self.depends_on {
            crate::path::name(name)?;
            if !seen.insert(name) {
                return Err(format!("duplicate dependency: {name}"));
            }
        }
        for note in &self.note {
            crate::path::text(note)?;
        }
        for package in self.packages.apt.iter().chain(&self.packages.pacman) {
            crate::path::text(package)?;
            if package.starts_with('-')
                || package.chars().any(|c| c.is_whitespace() || c.is_control())
            {
                return Err(format!("invalid package name: {package}"));
            }
        }
        for path in &self.env.path_prepend {
            crate::path::destination(path)?;
        }
        if let Some(script) = &self.env.path_export {
            crate::path::text(script)?;
        }
        for (i, operation) in self.operations.iter().enumerate() {
            operation.validate().map_err(|e| {
                format!("operations[{}] {}: {e}", i + 1, operation.kind())
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn definitions_reject_unknown_keys_and_invalid_fields_at_every_level() {
        let invalid = [
            "name = 'a'\nextra = true",
            "name = '../a'",
            "name = 'a'\ndepends_on = ['b','b']",
            "name = 'a'\n[env]\nextra = true",
            "name = 'a'\n[packages]\nextra = []",
            "name = 'a'\n[[operations]]\nkind='shell'\nrun=''",
            "name = 'a'\n[[operations]]\nkind='shell'\nrun='true'\ncheck=''",
            "name = 'a'\n[[operations]]\nkind='shell'\nrun='true'\nsrc='file'",
            "name = 'a'\n[[operations]]\nkind='directory'\npath='relative'",
            "name = 'a'\n[[operations]]\nkind='directory'\npath='~/ok'\nmode='888'",
            "name = 'a'\n[[operations]]\nkind='file'\nsrc='../outside'\ndst='~/ok'",
            "name = 'a'\n[[operations]]\nkind='block'\nsrc='file'\ndst='~/ok'\nmarker='bad>>>'",
            "name = 'a'\n[[operations]]\nkind='shell'\nrun='true'\nwhen={arch=[]}",
            "name = 'a'\n[[operations]]\nkind='shell'\nrun='true'\nwhen={arch=['x86_64','x86_64']}",
            "name = 'a'\n[[operations]]\nkind='shell'\nrun='true'\nwhen={unknown=true}",
        ];
        for input in invalid {
            let invalid = match toml::from_str::<super::Module>(input) {
                Ok(module) => module.validate().is_err(),
                Err(_) => true,
            };
            assert!(invalid, "accepted {input}");
        }
    }
}
