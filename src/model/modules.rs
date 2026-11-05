pub(crate) fn load(
    root: &std::path::Path,
) -> Result<
    std::collections::BTreeMap<String, crate::model::Module>,
    crate::error::MoiError,
> {
    let mut modules = std::collections::BTreeMap::new();
    for entry in
        std::fs::read_dir(root).map_err(|e| crate::error::MoiError::io(root, e))?
    {
        let path = entry
            .map_err(|e| crate::error::MoiError::io(root, e))?
            .path();
        let Some(meta) = crate::path::metadata(&path)? else {
            continue;
        };
        if !meta.is_dir() && !meta.file_type().is_symlink() {
            continue;
        }
        let file = path.join("module.toml");
        let content = std::fs::read_to_string(&file)
            .map_err(|e| crate::error::MoiError::io(&file, e))?;
        let mut module: crate::model::Module =
            toml::from_str(&content).map_err(|source| {
                crate::error::MoiError::Toml {
                    path: file.clone(),
                    source,
                }
            })?;
        module.validate().map_err(|e| {
            crate::error::MoiError::config(format!("{}: {e}", file.display()))
        })?;
        if path.file_name().and_then(|s| s.to_str()) != Some(&module.name) {
            return Err(crate::error::MoiError::config(format!(
                "{}: name must match directory",
                file.display()
            )));
        }
        module.path = path;
        modules.insert(module.name.clone(), module);
    }
    // Validate the entire environment before applying the requested selection.
    order(&modules, &modules.keys().cloned().collect())?;
    Ok(modules)
}

pub(crate) fn resolve<'a>(
    modules: &'a std::collections::BTreeMap<String, crate::model::Module>,
    requested: &[String],
) -> Result<Vec<&'a crate::model::Module>, crate::error::MoiError> {
    order(modules, &modules.keys().cloned().collect())?;
    let mut selected = std::collections::BTreeSet::new();
    let mut pending = if requested.is_empty() {
        modules.keys().cloned().collect()
    } else {
        requested.to_vec()
    };
    while let Some(name) = pending.pop() {
        let module = modules.get(&name).ok_or_else(|| {
            crate::error::MoiError::config(format!("unknown module: {name}"))
        })?;
        if selected.insert(name) {
            pending.extend(module.depends_on.iter().cloned());
        }
    }
    order(modules, &selected)
}

fn order<'a>(
    modules: &'a std::collections::BTreeMap<String, crate::model::Module>,
    selected: &std::collections::BTreeSet<String>,
) -> Result<Vec<&'a crate::model::Module>, crate::error::MoiError> {
    let mut remaining = std::collections::BTreeMap::new();
    let mut users: std::collections::BTreeMap<&str, Vec<&str>> =
        std::collections::BTreeMap::new();
    for name in selected {
        let module = &modules[name];
        for dep in &module.depends_on {
            if !modules.contains_key(dep) {
                return Err(crate::error::MoiError::config(format!(
                    "{name}: unknown dependency: {dep}"
                )));
            }
            users.entry(dep).or_default().push(name);
        }
        remaining.insert(name.as_str(), module.depends_on.len());
    }
    let mut ready: std::collections::BTreeSet<_> = remaining
        .iter()
        .filter_map(|(name, n)| (*n == 0).then_some(*name))
        .collect();
    let mut result = Vec::new();
    while let Some(name) = ready.pop_first() {
        result.push(&modules[name]);
        if let Some(users) = users.get(name) {
            for user in users {
                if let Some(n) = remaining.get_mut(user) {
                    *n -= 1;
                    if *n == 0 {
                        ready.insert(user);
                    }
                }
            }
        }
    }
    if result.len() != selected.len() {
        return Err(crate::error::MoiError::config("dependency cycle"));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn modules(
        text: &[(&str, &[&str])],
    ) -> std::collections::BTreeMap<String, crate::model::Module> {
        text.iter()
            .map(|(name, deps)| {
                let mut m: crate::model::Module =
                    toml::from_str(&format!("name = {name:?}")).unwrap();
                m.depends_on = deps.iter().map(|d| d.to_string()).collect();
                (name.to_string(), m)
            })
            .collect()
    }
    #[test]
    fn chooses_smallest_ready_module() {
        let m = modules(&[("a", &["z"]), ("b", &[]), ("z", &[])]);
        assert_eq!(
            resolve(&m, &[])
                .unwrap()
                .iter()
                .map(|m| m.name.as_str())
                .collect::<Vec<_>>(),
            ["b", "z", "a"]
        );
    }
    #[test]
    fn rejects_unselected_invalid_dependencies_and_cycles() {
        for m in [
            modules(&[("a", &[]), ("b", &["missing"])]),
            modules(&[("a", &[]), ("b", &["c"]), ("c", &["b"])]),
        ] {
            assert!(resolve(&m, &["a".into()]).is_err());
        }
    }
}
