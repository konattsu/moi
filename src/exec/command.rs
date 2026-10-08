pub(crate) fn plan(command: &crate::model::Command, index: usize, ignore_unless: bool) {
    describe_requires(command);
    describe_unless(command, index, ignore_unless);
    describe_run(command, index);
}

pub(crate) fn apply(
    module: &crate::model::Module,
    command: &crate::model::Command,
    index: usize,
    env: &mut crate::exec::environment::ExecutionEnv,
    ignore_unless: bool,
    reporter: &mut crate::reporter::Reporter,
) -> std::result::Result<(), crate::error::MoiError> {
    log_requires(command, reporter);
    for required in command.requires() {
        ensure_command(required, env)?;
    }
    if let Some(unless) = command.unless().filter(|_| !ignore_unless) {
        reporter.log(format!(
            "unless{} command[{index}]",
            command.platform().label()
        ));
        if crate::exec::shell::run_bash_status(unless, module.path(), env, reporter)?
            .success()
        {
            reporter.log(format!("skip command[{index}]"));
            return Ok(());
        }
    }
    reporter.log(format!(
        "run{} command[{index}]",
        command.platform().label()
    ));
    crate::exec::shell::run_bash(command.run(), module.path(), env, reporter)?;

    Ok(())
}

fn log_requires(
    command: &crate::model::Command,
    reporter: &mut crate::reporter::Reporter,
) {
    for required in command.requires() {
        reporter.log(format!("require {required}"));
    }
}

fn describe_requires(command: &crate::model::Command) {
    for required in command.requires() {
        crate::output!("    require {required}");
    }
}

fn describe_unless(command: &crate::model::Command, index: usize, ignore_unless: bool) {
    if command.unless().is_some() && !ignore_unless {
        crate::output!("    unless{} command[{index}]", command.platform().label());
    }
}

fn describe_run(command: &crate::model::Command, index: usize) {
    crate::output!("    run{} command[{index}]", command.platform().label());
}

fn ensure_command(
    command: &str,
    env: &mut crate::exec::environment::ExecutionEnv,
) -> std::result::Result<(), crate::error::MoiError> {
    if crate::exec::shell::which(command, env) {
        return Ok(());
    }
    if matches!(command, "node" | "npm" | "npx") {
        load_nvm_environment(env)?;
        if crate::exec::shell::which(command, env) {
            return Ok(());
        }
    }
    Err(crate::error::MoiError::config(format!(
        "required command not found: {command}"
    )))
}

fn load_nvm_environment(
    env: &mut crate::exec::environment::ExecutionEnv,
) -> std::result::Result<(), crate::error::MoiError> {
    let script = crate::path::home_dir()?.join(".nvm/nvm.sh");
    if !script.is_file() {
        return Ok(());
    }
    let mut process = std::process::Command::new("bash");
    process
        .arg("-c")
        .arg(". \"$HOME/.nvm/nvm.sh\" >/dev/null 2>&1 && env -0");
    env.apply_to(&mut process);
    let output = process
        .output()
        .map_err(|source| crate::error::MoiError::io(&script, source))?;
    if !output.status.success() {
        return Ok(());
    }
    for entry in output.stdout.split(|byte| *byte == 0) {
        if let Some(index) = entry.iter().position(|byte| *byte == b'=') {
            let key = String::from_utf8_lossy(&entry[..index]);
            if key == "PATH" {
                env.set_path(String::from_utf8_lossy(&entry[index + 1..]).to_string());
            }
        }
    }
    Ok(())
}
