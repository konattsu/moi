pub(crate) fn capture(
    script: &str,
    cwd: &std::path::Path,
    env: &crate::exec::environment::ExecutionEnv,
    strict: bool,
) -> Result<std::process::Output, crate::error::MoiError> {
    let mut command = std::process::Command::new("bash");
    command.args(["--noprofile", "--norc"]);
    if strict {
        command.args(["-e", "-o", "pipefail"]);
    }
    command.arg("-c").arg(script).current_dir(cwd);
    env.apply_to(&mut command);
    command
        .output()
        .map_err(|e| crate::error::MoiError::io(cwd, e))
}

pub(crate) fn check(
    script: &str,
    cwd: &std::path::Path,
    env: &crate::exec::environment::ExecutionEnv,
    reporter: &mut crate::reporter::Reporter,
) -> Result<bool, crate::error::MoiError> {
    let output = capture(script, cwd, env, false).map_err(|e| e.context("check"))?;
    reporter.child_output(&output.stdout);
    reporter.child_output(&output.stderr);
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(crate::error::MoiError::config(format!(
            "check: judgment failed ({}): stdout: {} stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))),
    }
}

pub(crate) fn run(
    script: &str,
    cwd: &std::path::Path,
    env: &crate::exec::environment::ExecutionEnv,
    reporter: &mut crate::reporter::Reporter,
) -> Result<(), crate::error::MoiError> {
    let mut command = std::process::Command::new("bash");
    command
        .args(["--noprofile", "--norc", "-e", "-o", "pipefail", "-c"])
        .arg(script)
        .current_dir(cwd);
    env.apply_to(&mut command);
    run_command(&mut command, reporter)
}

pub(crate) fn run_command(
    command: &mut std::process::Command,
    reporter: &mut crate::reporter::Reporter,
) -> Result<(), crate::error::MoiError> {
    crate::exec::environment::sanitize(command);
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| crate::error::MoiError::io(command.get_program(), e))?;
    let (tx, rx) = std::sync::mpsc::channel();
    fn reader<R: std::io::Read + Send + 'static>(
        reader: R,
        tx: std::sync::mpsc::Sender<Result<Vec<u8>, std::io::Error>>,
    ) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            use std::io::BufRead;
            let mut reader = std::io::BufReader::new(reader);
            loop {
                let mut bytes = Vec::new();
                match reader.read_until(b'\n', &mut bytes) {
                    Ok(0) => break,
                    Ok(_) => {
                        if tx.send(Ok(bytes)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        break;
                    }
                }
            }
        })
    }
    let out = reader(child.stdout.take().expect("piped stdout"), tx.clone());
    let err = reader(child.stderr.take().expect("piped stderr"), tx);
    let mut read_error = None;
    for bytes in rx {
        match bytes {
            Ok(bytes) => reporter.child_output(&bytes),
            Err(e) => read_error = Some(e),
        }
    }
    let _ = out.join();
    let _ = err.join();
    let status = child
        .wait()
        .map_err(|e| crate::error::MoiError::io(command.get_program(), e))?;
    if !status.success() {
        return Err(crate::error::MoiError::CommandExit(
            status.code().unwrap_or(1),
        ));
    }
    if let Some(e) = read_error {
        return Err(crate::error::MoiError::io(command.get_program(), e));
    }
    Ok(())
}

pub(crate) fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
