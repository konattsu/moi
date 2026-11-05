#[derive(Debug, clap::Parser)]
#[command(name = "moi")]
#[command(about = "moi module planner/applicator")]
#[command(version)]
pub struct Cli {
    #[command(flatten)]
    pub(crate) settings: SettingsArgs,
    #[command(flatten)]
    pub(crate) output: OutputArgs,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SettingsArgs {
    #[arg(short = 'e', long, global = true, help = "Target environment name")]
    pub(crate) environment: Option<String>,
    #[arg(
        long = "folder-name",
        global = true,
        help = "Repository-relative environments folder"
    )]
    pub(crate) folder_name: Option<String>,
    #[arg(
        long,
        global = true,
        help = "Repository source URL or file:// local repository path"
    )]
    pub(crate) source: Option<String>,
}

#[derive(Debug, clap::Args)]
pub(crate) struct OutputArgs {
    #[arg(
        long,
        global = true,
        conflicts_with = "verbose",
        help = "Suppress normal output"
    )]
    pub(crate) quiet: bool,
    #[arg(
        short,
        long,
        action = clap::ArgAction::Count,
        global = true,
        help = "Increase diagnostic output"
    )]
    pub(crate) verbose: u8,
}

#[derive(Debug, clap::Subcommand)]
pub(crate) enum Command {
    /// Show planned operations without changing files.
    Plan(RunArgs),
    /// Apply planned operations.
    Apply(ApplyArgs),
    /// Print a shell command that installs moi and runs plan/apply.
    #[command(name = "install-command")]
    Install(InstallCommandArgs),
    /// Upgrade moi to the latest release.
    Upgrade(UpgradeArgs),
}

#[derive(Debug, clap::Args)]
pub(crate) struct RunArgs {
    #[arg(long, help = "Show selected modules' notes")]
    pub(crate) show_note: bool,
    #[arg(help = "Module names to process")]
    pub(crate) modules: Vec<String>,
}

#[derive(Debug, clap::Args)]
pub(crate) struct ApplyArgs {
    #[command(flatten)]
    pub(crate) run: RunArgs,
    #[arg(
        long,
        help = "Upgrade system packages before installing missing packages"
    )]
    pub(crate) upgrade_packages: bool,
}

#[derive(Debug, clap::Args)]
pub(crate) struct InstallCommandArgs {
    #[arg(
        long,
        help = "Base URL or local directory containing the install script"
    )]
    pub(crate) install_source: Option<String>,
    #[arg(long, help = "Install script path relative to --install-source")]
    pub(crate) install_script: Option<String>,
    #[arg(
        value_enum,
        default_value_t = InstallOperationArg::Apply,
        help = "Operation passed to the installed moi binary"
    )]
    pub(crate) operation: InstallOperationArg,
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        help = "Arguments passed after the operation"
    )]
    pub(crate) args: Vec<String>,
}

#[derive(Debug, clap::Args)]
pub(crate) struct UpgradeArgs {
    #[arg(
        long,
        help = "Reinstall even when the latest version is already installed"
    )]
    pub(crate) force: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum InstallOperationArg {
    Plan,
    Apply,
}

impl Cli {
    pub fn stdout_settings(&self) -> crate::util::tracing::StdoutSettings {
        crate::util::tracing::StdoutSettings {
            quiet: self.output.quiet,
            verbose: self.output.verbose,
        }
    }
}

impl std::fmt::Display for InstallOperationArg {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            InstallOperationArg::Plan => "plan",
            InstallOperationArg::Apply => "apply",
        })
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    #[test]
    fn test_parse_settings_before_command() {
        let cli = super::Cli::parse_from(["moi", "--environment", "host", "plan"]);

        assert_eq!(cli.settings.environment.as_deref(), Some("host"));
    }

    #[test]
    fn test_parse_environment_short_flag() {
        let cli = super::Cli::parse_from(["moi", "-e", "host", "plan"]);

        assert_eq!(cli.settings.environment.as_deref(), Some("host"));
    }

    #[test]
    fn test_parse_global_settings_after_command() {
        let cli = super::Cli::parse_from(["moi", "plan", "--environment", "host"]);

        assert_eq!(cli.settings.environment.as_deref(), Some("host"));
    }

    #[test]
    fn rejects_removed_options() {
        for option in ["--platform", "--ignore-unless", "--show-followups"] {
            assert!(super::Cli::try_parse_from(["moi", "apply", option]).is_err());
        }
    }

    #[test]
    fn test_parse_global_output_flags_after_command() {
        let cli = super::Cli::parse_from(["moi", "plan", "--quiet"]);

        assert!(cli.output.quiet);
    }

    #[test]
    fn test_count_verbose_flags() {
        let cli = super::Cli::parse_from(["moi", "-vv", "plan"]);

        assert_eq!(cli.output.verbose, 2);
    }

    #[test]
    fn test_reject_quiet_verbose_conflict() {
        let error =
            super::Cli::try_parse_from(["moi", "--quiet", "-v", "plan"]).unwrap_err();

        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn test_parse_upgrade_packages() {
        let cli = super::Cli::parse_from(["moi", "apply", "--upgrade-packages"]);
        let super::Command::Apply(args) = cli.command else {
            panic!("expected apply command");
        };

        assert!(args.upgrade_packages);
    }

    #[test]
    fn test_parse_upgrade() {
        let cli = super::Cli::parse_from(["moi", "upgrade"]);
        let super::Command::Upgrade(args) = cli.command else {
            panic!("expected upgrade command");
        };

        assert!(!args.force);
    }

    #[test]
    fn test_parse_upgrade_force() {
        let cli = super::Cli::parse_from(["moi", "upgrade", "--force"]);
        let super::Command::Upgrade(args) = cli.command else {
            panic!("expected upgrade command");
        };

        assert!(args.force);
    }

    #[test]
    fn test_parse_install_command_defaults_to_apply() {
        let cli = super::Cli::parse_from([
            "moi",
            "install-command",
            "--environment",
            "devcontainer",
        ]);
        let super::Command::Install(args) = cli.command else {
            panic!("expected install-command");
        };

        assert_eq!(cli.settings.environment.as_deref(), Some("devcontainer"));
        assert_eq!(args.operation, super::InstallOperationArg::Apply);
        assert_eq!(args.install_script, None);
    }

    #[test]
    fn test_parse_install_command_plan_args() {
        let cli = super::Cli::parse_from([
            "moi",
            "install-command",
            "--environment",
            "devcontainer",
            "plan",
            "--show-note",
            "nvim",
        ]);
        let super::Command::Install(args) = cli.command else {
            panic!("expected install-command");
        };

        assert_eq!(args.operation, super::InstallOperationArg::Plan);
        assert_eq!(args.args, ["--show-note", "nvim"]);
    }

    #[test]
    fn test_parse_install_command_apply_args() {
        let cli = super::Cli::parse_from([
            "moi",
            "install-command",
            "--environment",
            "devcontainer",
            "apply",
            "--upgrade-packages",
        ]);
        let super::Command::Install(args) = cli.command else {
            panic!("expected install-command");
        };

        assert_eq!(args.operation, super::InstallOperationArg::Apply);
        assert_eq!(args.args, ["--upgrade-packages"]);
    }
}
