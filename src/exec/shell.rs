pub(in crate::exec) fn run_bash(
    command: &str,
    cwd: &std::path::Path,
    env: &crate::exec::environment::ExecutionEnv,
    reporter: &mut crate::reporter::Reporter,
) -> std::result::Result<(), crate::error::MoiError> {
    let status = run_bash_status(command, cwd, env, reporter)?;
    if !status.success() {
        return Err(crate::error::MoiError::CommandExit(
            status.code().unwrap_or(1),
        ));
    }
    Ok(())
}

pub(in crate::exec) fn run_bash_status(
    command: &str,
    cwd: &std::path::Path,
    env: &crate::exec::environment::ExecutionEnv,
    reporter: &mut crate::reporter::Reporter,
) -> std::result::Result<std::process::ExitStatus, crate::error::MoiError> {
    let mut process = std::process::Command::new("bash");
    process
        .arg("-euo")
        .arg("pipefail")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    env.apply_to(&mut process);

    let mut child = process
        .spawn()
        .map_err(|source| crate::error::MoiError::io(cwd, source))?;
    let stdout = child.stdout.take().expect("stdout was configured as piped");
    let stderr = child.stderr.take().expect("stderr was configured as piped");
    let (sender, receiver) = std::sync::mpsc::channel();
    let stdout_reader = spawn_reader(stdout, sender.clone());
    let stderr_reader = spawn_reader(stderr, sender);

    while let Ok(line) = receiver.recv() {
        reporter.log(line);
    }

    let _ = stdout_reader.join();
    let _ = stderr_reader.join();
    let status = child
        .wait()
        .map_err(|source| crate::error::MoiError::io(cwd, source))?;
    Ok(status)
}

fn spawn_reader<R>(
    reader: R,
    sender: std::sync::mpsc::Sender<String>,
) -> std::thread::JoinHandle<()>
where
    R: std::io::Read + Send + 'static,
{
    std::thread::spawn(move || {
        use std::io::BufRead;

        let mut reader = std::io::BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let line = line.trim_end_matches(['\r', '\n']).to_string();
                    if sender.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    })
}

pub(in crate::exec) fn which(
    command: &str,
    env: &crate::exec::environment::ExecutionEnv,
) -> bool {
    std::env::split_paths(env.path()).any(|dir| {
        let candidate = dir.join(command);
        candidate.is_file() && is_executable(&candidate)
    })
}

fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
