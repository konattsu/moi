pub(crate) fn install(
    packages: &[String],
    platform: crate::platform::Platform,
    repo_root: &std::path::Path,
    env: &crate::exec::environment::ExecutionEnv,
    upgrade: bool,
    reporter: &mut crate::reporter::Reporter,
) -> std::result::Result<(), crate::error::MoiError> {
    if packages.is_empty() {
        return Ok(());
    }
    let commands = package_commands(packages, platform, upgrade);
    let total = commands.len();
    for (index, command) in commands.into_iter().enumerate() {
        reporter.start_step(index + 1, total, &command.display);
        if let Err(error) =
            crate::exec::shell::run_bash(&command.run, repo_root, env, reporter)
        {
            reporter.fail_step(&error);
            return Err(error);
        }
        reporter.finish_step();
    }
    Ok(())
}

pub(crate) fn descriptions(
    packages: &[String],
    platform: crate::platform::Platform,
    upgrade: bool,
) -> Vec<String> {
    package_commands(packages, platform, upgrade)
        .into_iter()
        .map(|command| command.display)
        .collect()
}

fn package_commands(
    packages: &[String],
    platform: crate::platform::Platform,
    upgrade: bool,
) -> Vec<PackageCommand> {
    let quoted = packages
        .iter()
        .map(|package| crate::exec::path::shell_quote(package))
        .collect::<Vec<_>>()
        .join(" ");
    match platform {
        crate::platform::Platform::Debian => {
            let mut commands = vec![PackageCommand::new("sudo apt update".to_string())];
            if upgrade {
                commands.push(PackageCommand::new("sudo apt upgrade -y".to_string()));
            }
            commands.push(PackageCommand::with_display(
                format!("sudo apt install -y {quoted}"),
                format!("apt install ({} packages)", packages.len()),
            ));
            commands
        }
        crate::platform::Platform::Arch => {
            vec![pacman_install_command(&quoted, upgrade)]
        }
    }
}

struct PackageCommand {
    run: String,
    display: String,
}

impl PackageCommand {
    fn new(run: String) -> Self {
        let display = run.strip_prefix("sudo ").unwrap_or(&run).to_string();
        Self { run, display }
    }

    fn with_display(run: String, display: String) -> Self {
        Self { run, display }
    }
}

fn pacman_install_command(packages: &str, upgrade: bool) -> PackageCommand {
    let flags = if upgrade { "-Syu" } else { "-S" };
    PackageCommand::new(format!(
        "sudo pacman {flags} --needed --noconfirm {packages}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pacman_install_command_without_upgrade() {
        let command = pacman_install_command("'git'", false);

        assert_eq!(command.run, "sudo pacman -S --needed --noconfirm 'git'");
        assert_eq!(command.display, "pacman -S --needed --noconfirm 'git'");
    }

    #[test]
    fn test_pacman_install_command_with_upgrade() {
        let command = pacman_install_command("'git'", true);

        assert_eq!(command.run, "sudo pacman -Syu --needed --noconfirm 'git'");
        assert_eq!(command.display, "pacman -Syu --needed --noconfirm 'git'");
    }
}
