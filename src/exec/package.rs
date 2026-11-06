pub(crate) fn install(
    packages: &[String],
    distro: crate::platform::Distro,
    env: &crate::exec::environment::ExecutionEnv,
    upgrade: bool,
    reporter: &mut crate::reporter::Reporter,
) -> Result<(), crate::error::MoiError> {
    let total = match distro {
        crate::platform::Distro::Debian => 3 + usize::from(upgrade),
        crate::platform::Distro::Arch => 2,
    };
    reporter.start_step(1, total, "query installed packages");
    let mut missing = Vec::new();
    for package in packages {
        let mut query = match distro {
            crate::platform::Distro::Debian => {
                let mut c = std::process::Command::new("dpkg-query");
                c.args(["-W", "-f=${Status}", "--", package]);
                c
            }
            crate::platform::Distro::Arch => {
                let mut c = std::process::Command::new("pacman");
                c.args(["-Q", "--", package]);
                c
            }
        };
        env.apply_to(&mut query);
        let output = query
            .output()
            .map_err(|e| crate::error::MoiError::io(query.get_program(), e))?;
        match output.status.code() {
            Some(0) => {
                if distro == crate::platform::Distro::Debian
                    && output.stdout != b"install ok installed"
                {
                    missing.push(package.clone());
                }
            }
            Some(1) => missing.push(package.clone()),
            _ => {
                return Err(crate::error::MoiError::config(format!(
                    "package query failed: {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                )));
            }
        }
    }
    reporter.finish_step("done");
    if missing.is_empty() && !upgrade {
        if distro == crate::platform::Distro::Debian {
            reporter.start_step(2, total, "fetch package index");
            reporter.finish_step("skipped");
        }
        reporter.start_step(total, total, "install packages");
        reporter.finish_step("unchanged");
        return Ok(());
    }
    fn run(
        program: &str,
        args: &[&str],
        packages: &[String],
        env: &crate::exec::environment::ExecutionEnv,
        reporter: &mut crate::reporter::Reporter,
    ) -> Result<(), crate::error::MoiError> {
        // Linux geteuid is queried without changing the invoking user's environment.
        let root = std::fs::read_to_string("/proc/self/status")
            .map_err(|e| crate::error::MoiError::io("/proc/self/status", e))?
            .lines()
            .find(|l| l.starts_with("Uid:"))
            .and_then(|l| l.split_whitespace().nth(2))
            .is_some_and(|uid| uid == "0");
        let mut command = if root {
            std::process::Command::new(program)
        } else {
            let mut c = std::process::Command::new("sudo");
            c.arg(program);
            c
        };
        command.args(args);
        if !packages.is_empty() {
            command.arg("--").args(packages);
        }
        env.apply_to(&mut command);
        reporter.log(format!("packages: {program} {}", args.join(" ")));
        crate::exec::shell::run_command(&mut command, reporter)
    }
    match distro {
        crate::platform::Distro::Debian => {
            reporter.start_step(2, total, "fetch package index");
            run("apt-get", &["update"], &[], env, reporter)?;
            reporter.finish_step("done");
            if upgrade {
                reporter.start_step(3, total, "upgrade packages");
                run(
                    "apt-get",
                    &["upgrade", "--with-new-pkgs", "-y"],
                    &[],
                    env,
                    reporter,
                )?;
                reporter.finish_step("done");
            }
            reporter.start_step(total, total, "install packages");
            if !missing.is_empty() {
                run("apt-get", &["install", "-y"], &missing, env, reporter)?;
                reporter.finish_step("done");
            } else {
                reporter.finish_step("unchanged");
            }
        }
        crate::platform::Distro::Arch => {
            reporter.start_step(
                2,
                total,
                if upgrade {
                    "refresh and upgrade packages"
                } else {
                    "install packages"
                },
            );
            run(
                "pacman",
                &[
                    if upgrade { "-Syu" } else { "-S" },
                    "--needed",
                    "--noconfirm",
                ],
                &missing,
                env,
                reporter,
            )?;
            reporter.finish_step("done");
        }
    }
    Ok(())
}
