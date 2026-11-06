pub fn run(cli: crate::cli::Cli) -> Result<(), crate::error::MoiError> {
    let mut reporter = crate::reporter::Reporter::new(cli.stdout_settings());
    let result = match &cli.command {
        crate::cli::Command::Plan(args) => run_environment(
            &cli.settings,
            args,
            false,
            false,
            cli.output.quiet,
            &mut reporter,
        ),
        crate::cli::Command::Apply(args) => run_environment(
            &cli.settings,
            &args.run,
            true,
            args.upgrade_packages,
            cli.output.quiet,
            &mut reporter,
        ),
        crate::cli::Command::Install(args) => install_command(&cli.settings, args),
        crate::cli::Command::Upgrade(args) => crate::upgrade::run(args, &mut reporter),
    };
    if result.is_err() {
        reporter.failure();
    }
    result
}

fn run_environment(
    settings: &crate::cli::SettingsArgs,
    args: &crate::cli::RunArgs,
    apply: bool,
    upgrade_packages: bool,
    quiet: bool,
    reporter: &mut crate::reporter::Reporter,
) -> Result<(), crate::error::MoiError> {
    let settings =
        crate::config::Settings::resolve(crate::config::SettingsOverrides {
            environment: settings.environment.as_deref(),
            folder_name: settings.folder_name.as_deref(),
            source: settings.source.as_deref(),
        })?;
    if apply {
        reporter.start_stage(1, 3, "repository clone");
        reporter.start_step(
            1,
            2,
            if settings.source().starts_with("file:") {
                "open local repository"
            } else {
                "git clone"
            },
        );
    }
    let repo = Repository::open(settings.source(), reporter)?;
    if apply {
        reporter.finish_step("done");
        reporter.start_step(2, 2, "validate modules");
    }
    let root = repo
        .path
        .join(settings.folder_name())
        .join(settings.environment())
        .join("modules");
    let actual = std::fs::canonicalize(&root)
        .map_err(|e| crate::error::MoiError::io(&root, e))?;
    let repo_root = std::fs::canonicalize(&repo.path)
        .map_err(|e| crate::error::MoiError::io(&repo.path, e))?;
    if !actual.starts_with(&repo_root) {
        return Err(crate::error::MoiError::config(
            "environment must remain inside the repository",
        ));
    }
    let modules = crate::model::modules::load(&root)?;
    let ordered = crate::model::modules::resolve(&modules, &args.modules)?;
    let platform = crate::platform::detect()?;
    tracing::debug!(?platform, "detected execution environment");
    let mut prepared = Vec::new();
    for module in &ordered {
        for (index, operation) in module.operations.iter().enumerate() {
            let label = format!(
                "{} operations[{}] {}: preflight",
                module.name,
                index + 1,
                operation.kind()
            );
            prepared.push(
                crate::exec::standard::Prepared::new(
                    module,
                    index + 1,
                    operation,
                    platform,
                    &repo_root,
                )
                .map_err(|e| e.context(label))?,
            );
        }
    }
    crate::exec::validation::validate(&prepared)?;
    let packages: Vec<_> = ordered
        .iter()
        .flat_map(|m| match platform.distro {
            crate::platform::Distro::Debian => &m.packages.apt,
            crate::platform::Distro::Arch => &m.packages.pacman,
        })
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    if apply {
        reporter.finish_step("done");
        reporter.finish_stage("done");
        reporter.start_stage(2, 3, "system packages");
        let mut env = crate::exec::environment::ExecutionEnv::new();
        crate::exec::package::install(
            &packages,
            platform.distro,
            &env,
            upgrade_packages,
            reporter,
        )
        .map_err(|e| e.context("system packages"))?;
        reporter.finish_stage("done");
        reporter.start_stage(3, 3, "apply modules");
        let mut counts = [0usize; 4];
        for (index, module) in ordered.iter().enumerate() {
            reporter.start_step(index + 1, ordered.len(), &module.name);
            env.prepend(&module.env.path_prepend)
                .map_err(|e| e.context(format!("{} env.path_prepend", module.name)))?;
            for operation in prepared.iter().filter(|o| std::ptr::eq(o.module, *module))
            {
                let label = operation.label();
                reporter.log(format!("{label}: applying"));
                let outcome = operation
                    .apply(&env, reporter)
                    .map_err(|e| e.context(format!("{label}: failed")))?;
                reporter.log(format!("{label}: {}", outcome.name()));
                counts[match outcome {
                    crate::exec::standard::Outcome::Changed => 0,
                    crate::exec::standard::Outcome::Unchanged => 1,
                    crate::exec::standard::Outcome::NotApplicable => 2,
                    crate::exec::standard::Outcome::Executed => 3,
                }] += 1;
            }
            if let Some(export) = &module.env.path_export {
                reporter.log(format!("{} env.path_export: applying", module.name));
                env.export(export, &module.path, reporter).map_err(|e| {
                    e.context(format!("{} env.path_export", module.name))
                })?;
                reporter.log(format!("{} env.path_export: done", module.name));
            }
            reporter.finish_step("done");
        }
        reporter.finish_stage("done");
        reporter.log(format!(
            "{}: changed={} unchanged={} not-applicable={} executed={}",
            settings.environment(),
            counts[0],
            counts[1],
            counts[2],
            counts[3]
        ));
    } else {
        if !packages.is_empty() {
            crate::output!(
                "packages {:?}: {} (installation determined during apply)",
                platform.distro,
                packages.join(", ")
            );
        }
        for operation in &prepared {
            let state = if !operation.applicable {
                "not-applicable (when does not match)"
            } else if matches!(
                operation.operation,
                crate::model::Operation::Shell { .. }
            ) {
                "check/run determined during apply"
            } else if operation
                .satisfied()
                .map_err(|e| e.context(format!("{}: plan state", operation.label())))?
            {
                "unchanged"
            } else {
                "change planned"
            };
            crate::output!(
                "{}: {state}; environment={platform:?}; desired={}",
                operation.label(),
                desired(operation.operation)
            );
        }
        for module in &ordered {
            if module.env.path_export.is_some() {
                crate::output!(
                    "{} env.path_export: PATH obtained during apply",
                    module.name
                );
            }
        }
    }
    if args.show_note && !quiet {
        for module in &ordered {
            for note in &module.note {
                eprintln!("{}: {note}", module.name);
            }
        }
    }
    Ok(())
}

fn desired(operation: &crate::model::Operation) -> String {
    match operation {
        crate::model::Operation::Directory { mode, .. } => {
            format!("directory mode={mode:?}")
        }
        crate::model::Operation::File { src, mode, .. } => {
            format!("file src={src} mode={mode:?}")
        }
        crate::model::Operation::Symlink { target, .. } => {
            format!("symlink target={target}")
        }
        crate::model::Operation::Block { src, marker, .. } => {
            format!("block src={src} marker={marker}")
        }
        crate::model::Operation::Archive(a) => format!(
            "archive version={} policy={:?} include={:?}",
            a.version, a.policy, a.include
        ),
        crate::model::Operation::Shell { check, .. } => format!(
            "shell check={}",
            if check.is_some() {
                "declared"
            } else {
                "always run"
            }
        ),
    }
}

struct Repository {
    path: std::path::PathBuf,
    _temporary: Option<tempfile::TempDir>,
}
impl Repository {
    fn open(
        source: &str,
        reporter: &mut crate::reporter::Reporter,
    ) -> Result<Self, crate::error::MoiError> {
        if source.starts_with("file:") {
            let path = url::Url::parse(source)
                .map_err(|e| crate::error::MoiError::config(e.to_string()))?
                .to_file_path()
                .map_err(|_| crate::error::MoiError::config("invalid file URL"))?;
            return Ok(Self {
                path,
                _temporary: None,
            });
        }
        let temporary = tempfile::Builder::new()
            .prefix("moi-repository-")
            .tempdir()
            .map_err(|e| crate::error::MoiError::io("temporary checkout", e))?;
        let mut command = std::process::Command::new("git");
        command
            .args(["clone", "--depth", "1", "--branch"])
            .arg(std::env::var_os("MOI_BRANCH").unwrap_or_else(|| "main".into()))
            .arg("--")
            .arg(source)
            .arg(temporary.path());
        crate::exec::environment::ExecutionEnv::new().apply_to(&mut command);
        crate::exec::shell::run_command(&mut command, reporter)
            .map_err(|e| e.context("repository clone"))?;
        Ok(Self {
            path: temporary.path().into(),
            _temporary: Some(temporary),
        })
    }
}

fn install_command(
    settings: &crate::cli::SettingsArgs,
    args: &crate::cli::InstallCommandArgs,
) -> Result<(), crate::error::MoiError> {
    let settings = crate::config::InstallCommandSettings::resolve(
        crate::config::InstallCommandOverrides {
            environment: settings.environment.as_deref(),
            folder_name: settings.folder_name.as_deref(),
            source: settings.source.as_deref(),
            install_source: args.install_source.as_deref(),
            install_script: args.install_script.as_deref(),
        },
    )?;
    let script = if settings.install_source().starts_with("https://") {
        let mut url = crate::path::https(settings.install_source())
            .map_err(crate::error::MoiError::config)?;
        url.set_path(&format!(
            "{}/{}",
            url.path().trim_end_matches('/'),
            settings.install_script()
        ));
        format!("curl -fsSL {}", crate::exec::shell::quote(url.as_str()))
    } else {
        let source = if settings.install_source().starts_with("file:") {
            url::Url::parse(settings.install_source())
                .map_err(|e| crate::error::MoiError::config(e.to_string()))?
                .to_file_path()
                .map_err(|_| {
                    crate::error::MoiError::config("invalid install source URL")
                })?
        } else {
            std::path::PathBuf::from(settings.install_source())
        };
        format!(
            "cat {}",
            crate::exec::shell::quote(
                &source.join(settings.install_script()).to_string_lossy()
            )
        )
    };
    let args = std::iter::once(args.operation.to_string())
        .chain(args.args.iter().cloned())
        .map(|s| crate::exec::shell::quote(&s))
        .collect::<Vec<_>>()
        .join(" ");
    crate::output!(
        "{script} | MOI_ENVIRONMENT={} MOI_FOLDER_NAME={} MOI_SOURCE={} bash -s -- {args}",
        crate::exec::shell::quote(settings.environment()),
        crate::exec::shell::quote(settings.folder_name()),
        crate::exec::shell::quote(settings.source())
    );
    Ok(())
}
