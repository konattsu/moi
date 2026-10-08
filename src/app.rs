pub fn run(cli: crate::cli::Cli) -> std::result::Result<(), crate::error::MoiError> {
    match &cli.command {
        crate::cli::Command::Plan(args) => run_plan(&cli.settings, args),
        crate::cli::Command::Apply(args) => {
            run_apply(&cli.settings, args, cli.stdout_settings())
        }
        crate::cli::Command::Install(args) => run_install_command(&cli.settings, args),
        crate::cli::Command::Upgrade(args) => crate::upgrade::run(args),
    }
}

fn run_plan(
    settings_args: &crate::cli::SettingsArgs,
    args: &crate::cli::RunArgs,
) -> std::result::Result<(), crate::error::MoiError> {
    let context = prepare_run_context(settings_args, args)?;
    let ordered_modules = context.ordered_modules();
    print_plan(&ordered_modules, context.platform, context.show_followups)
}

fn run_apply(
    settings_args: &crate::cli::SettingsArgs,
    args: &crate::cli::ApplyArgs,
    output_settings: crate::util::tracing::StdoutSettings,
) -> std::result::Result<(), crate::error::MoiError> {
    let context = prepare_run_context(settings_args, &args.run)?;
    let ordered_modules = context.ordered_modules();
    let mut reporter = crate::reporter::Reporter::new(output_settings);
    apply(
        &ordered_modules,
        context.repo.path(),
        context.platform,
        ApplyOptions {
            environment: &context.environment,
            show_followups: context.show_followups,
            ignore_unless: args.ignore_unless,
            upgrade_packages: args.upgrade_packages,
        },
        &mut reporter,
    )
}

fn run_install_command(
    settings_args: &crate::cli::SettingsArgs,
    args: &crate::cli::InstallCommandArgs,
) -> std::result::Result<(), crate::error::MoiError> {
    let settings = crate::config::InstallCommandSettings::resolve(
        crate::config::InstallCommandOverrides {
            environment: settings_args.environment.as_deref(),
            folder_name: settings_args.folder_name.as_deref(),
            source: settings_args.source.as_deref(),
            install_source: args.install_source.as_deref(),
            install_script: args.install_script.as_deref(),
        },
    )?;
    crate::output!("{}", build_install_command(&settings, args));
    Ok(())
}

fn prepare_run_context(
    settings_args: &crate::cli::SettingsArgs,
    args: &crate::cli::RunArgs,
) -> std::result::Result<RunContext, crate::error::MoiError> {
    let settings =
        crate::config::Settings::resolve(crate::config::SettingsOverrides {
            environment: settings_args.environment.as_deref(),
            folder_name: settings_args.folder_name.as_deref(),
            source: settings_args.source.as_deref(),
        })?;
    settings.write_if_missing()?;
    let repo = RepoCheckout::prepare(settings.source())?;
    let modules_dir = repo
        .path()
        .join(settings.folder_name())
        .join(settings.environment())
        .join("modules");
    let modules = crate::model::modules::load(&modules_dir)?;
    let ordered_modules = crate::model::modules::resolve(&modules, &args.modules)?
        .into_iter()
        .map(|module| module.name().to_string())
        .collect();
    let platform = resolve_platform(args)?;
    let show_followups = should_show_followups(args);
    Ok(RunContext {
        repo,
        modules,
        ordered_modules,
        platform,
        show_followups,
        environment: settings.environment().to_string(),
    })
}

fn build_install_command(
    settings: &crate::config::InstallCommandSettings,
    args: &crate::cli::InstallCommandArgs,
) -> String {
    let script_command = install_script_command(settings);
    let mut words = vec![args.operation.to_string()];
    words.extend(args.args.iter().cloned());
    let forwarded_args = words
        .iter()
        .map(|word| shell_word(word))
        .collect::<Vec<_>>()
        .join(" ");

    format!(
        "{} \\\n  | MOI_ENVIRONMENT={} MOI_FOLDER_NAME={} MOI_SOURCE={} bash -s -- {}",
        script_command,
        shell_word(settings.environment()),
        shell_word(settings.folder_name()),
        shell_word(settings.source()),
        forwarded_args
    )
}

fn install_script_command(settings: &crate::config::InstallCommandSettings) -> String {
    let script = settings.install_script().trim_start_matches('/');
    if settings.install_source().starts_with("https://") {
        let url = format!(
            "{}/{}",
            settings.install_source().trim_end_matches('/'),
            script
        );
        return format!("curl -fsSL {}", shell_word(&url));
    }

    let local_source = settings
        .install_source()
        .strip_prefix("file://")
        .unwrap_or_else(|| settings.install_source());
    let path = std::path::Path::new(local_source).join(script);
    format!("cat {}", shell_word(&path.display().to_string()))
}

fn shell_word(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_@%+=:,./-".contains(&byte))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

struct RunContext {
    repo: RepoCheckout,
    modules: std::collections::BTreeMap<String, crate::model::Module>,
    ordered_modules: Vec<String>,
    platform: crate::platform::Platform,
    show_followups: bool,
    environment: String,
}

struct RepoCheckout {
    path: std::path::PathBuf,
    _tempdir: Option<tempfile::TempDir>,
}

impl RunContext {
    fn ordered_modules(&self) -> Vec<&crate::model::Module> {
        self.ordered_modules
            .iter()
            .map(|name| {
                self.modules
                    .get(name)
                    .expect("resolved module names must exist")
            })
            .collect()
    }
}

impl RepoCheckout {
    fn prepare(source: &str) -> std::result::Result<Self, crate::error::MoiError> {
        if let Some(path) = source.strip_prefix("file://") {
            return Ok(Self {
                path: std::path::PathBuf::from(path),
                _tempdir: None,
            });
        }
        let tempdir = tempfile::Builder::new()
            .prefix("moi-")
            .tempdir()
            .map_err(|source| crate::error::MoiError::IoMessage(source.to_string()))?;
        let branch = std::env::var("MOI_BRANCH").unwrap_or_else(|_| "main".to_string());
        let status = std::process::Command::new("git")
            .arg("clone")
            .arg("--depth")
            .arg("1")
            .arg("--branch")
            .arg(&branch)
            .arg(source)
            .arg(tempdir.path())
            .status()
            .map_err(|source| crate::error::MoiError::IoMessage(source.to_string()))?;
        if !status.success() {
            return Err(crate::error::MoiError::CommandExit(
                status.code().unwrap_or(1),
            ));
        }
        Ok(Self {
            path: tempdir.path().to_path_buf(),
            _tempdir: Some(tempdir),
        })
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

fn resolve_platform(
    args: &crate::cli::RunArgs,
) -> std::result::Result<crate::platform::Platform, crate::error::MoiError> {
    match args.platform {
        crate::cli::PlatformArg::Auto => crate::platform::detect(),
        crate::cli::PlatformArg::Debian => Ok(crate::platform::Platform::Debian),
        crate::cli::PlatformArg::Arch => Ok(crate::platform::Platform::Arch),
    }
}

fn should_show_followups(args: &crate::cli::RunArgs) -> bool {
    if let Some(value) = args.followups_override() {
        return value;
    }
    std::env::var("MOI_FIRST_INSTALL").ok().as_deref() == Some("1")
}

fn collect_packages(
    modules: &[&crate::model::Module],
    platform: crate::platform::Platform,
) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut packages = Vec::new();
    for module in modules {
        for package in module.packages().for_platform(platform) {
            if seen.insert(package.clone()) {
                packages.push(package.clone());
            }
        }
    }
    packages
}

fn print_plan(
    modules: &[&crate::model::Module],
    platform: crate::platform::Platform,
    show_followups: bool,
) -> std::result::Result<(), crate::error::MoiError> {
    let packages = collect_packages(modules, platform);
    let stage_total =
        usize::from(!packages.is_empty()) + usize::from(!modules.is_empty());
    let mut stage_index = 0;
    if !packages.is_empty() {
        stage_index += 1;
        let descriptions =
            crate::exec::package::descriptions(&packages, platform, false);
        crate::output!("=> [{stage_index}/{stage_total}] Packages");
        for (index, description) in descriptions.iter().enumerate() {
            crate::output!("==> [{}/{}] {description}", index + 1, descriptions.len());
        }
        crate::output!();
    }
    let mut env = crate::exec::environment::ExecutionEnv::new();
    if !modules.is_empty() {
        stage_index += 1;
        crate::output!("=> [{stage_index}/{stage_total}] Modules");
        for (index, module) in modules.iter().enumerate() {
            crate::output!("==> [{}/{}] {}", index + 1, modules.len(), module.name());
            env.apply_module_env(module)?;
            plan_module_operations(module, platform)?;
        }
    }
    if show_followups {
        print_followups(modules);
    }
    Ok(())
}

fn apply(
    modules: &[&crate::model::Module],
    repo_root: &std::path::Path,
    platform: crate::platform::Platform,
    options: ApplyOptions<'_>,
    reporter: &mut crate::reporter::Reporter,
) -> std::result::Result<(), crate::error::MoiError> {
    let mut env = crate::exec::environment::ExecutionEnv::new();
    let packages = collect_packages(modules, platform);
    let stage_total =
        usize::from(!packages.is_empty()) + usize::from(!modules.is_empty());
    let mut stage_index = 0;
    if !packages.is_empty() {
        stage_index += 1;
        reporter.start_stage(stage_index, stage_total, "Packages");
        crate::exec::package::install(
            &packages,
            platform,
            repo_root,
            &env,
            options.upgrade_packages,
            reporter,
        )?;
        reporter.finish_stage();
    }
    if !modules.is_empty() {
        stage_index += 1;
        reporter.start_stage(stage_index, stage_total, "Modules");
        for (index, module) in modules.iter().enumerate() {
            reporter.start_step(index + 1, modules.len(), &module.name().to_string());
            let result = env.apply_module_env(module).and_then(|()| {
                apply_module_operations(
                    module,
                    platform,
                    &mut env,
                    options.ignore_unless,
                    reporter,
                )
            });
            if let Err(error) = result {
                reporter.fail_step(&error);
                return Err(error);
            }
            reporter.finish_step();
        }
        reporter.finish_stage();
    }
    if options.show_followups {
        print_followups(modules);
    }
    reporter.summary(options.environment, platform.name(), stage_total);
    Ok(())
}

struct ApplyOptions<'a> {
    environment: &'a str,
    show_followups: bool,
    ignore_unless: bool,
    upgrade_packages: bool,
}

fn plan_module_operations(
    module: &crate::model::Module,
    platform: crate::platform::Platform,
) -> std::result::Result<(), crate::error::MoiError> {
    for dir in module.dirs() {
        if dir.platform().matches(platform) {
            crate::exec::dir::plan(dir)?;
        }
    }
    for file in module.files() {
        if file.platform().matches(platform) {
            crate::exec::file::plan(module, file)?;
        }
    }
    for block in module.blocks() {
        if block.platform().matches(platform) {
            crate::exec::block::plan(module, block)?;
        }
    }
    for (index, command) in module.commands().iter().enumerate() {
        if command.platform().matches(platform) {
            crate::exec::command::plan(command, index + 1, false);
        }
    }
    Ok(())
}

fn apply_module_operations(
    module: &crate::model::Module,
    platform: crate::platform::Platform,
    env: &mut crate::exec::environment::ExecutionEnv,
    ignore_unless: bool,
    reporter: &mut crate::reporter::Reporter,
) -> std::result::Result<(), crate::error::MoiError> {
    for dir in module.dirs() {
        if dir.platform().matches(platform) {
            crate::exec::dir::apply(dir, reporter)?;
        }
    }
    for file in module.files() {
        if file.platform().matches(platform) {
            crate::exec::file::apply(module, file, reporter)?;
        }
    }
    for block in module.blocks() {
        if block.platform().matches(platform) {
            crate::exec::block::apply(module, block, reporter)?;
        }
    }
    for (index, command) in module.commands().iter().enumerate() {
        if command.platform().matches(platform) {
            crate::exec::command::apply(
                module,
                command,
                index + 1,
                env,
                ignore_unless,
                reporter,
            )?;
        }
    }
    Ok(())
}

fn print_followups(modules: &[&crate::model::Module]) {
    let followups = modules
        .iter()
        .flat_map(|module| module.followups())
        .collect::<Vec<_>>();
    if followups.is_empty() {
        return;
    }
    crate::output!();
    crate::output!("Follow-ups:");
    for followup in followups {
        crate::output!("  - {followup}");
    }
}
