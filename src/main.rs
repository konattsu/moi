fn main() {
    std::process::exit(env_main());
}

fn env_main() -> i32 {
    let cli = <moi::cli::Cli as clap::Parser>::parse();
    let _tracing_guard =
        moi::util::tracing::apply_tracing_settings("moi", cli.stdout_settings(), None);
    exit_code(moi::app::run(cli))
}

fn exit_code(result: std::result::Result<(), moi::error::MoiError>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(error) => {
            write_error(format_args!("{error}"));
            error.exit_code()
        }
    }
}

fn write_error(args: std::fmt::Arguments<'_>) {
    let mut stderr = std::io::stderr().lock();
    let _ = std::io::Write::write_fmt(&mut stderr, format_args!("error: {args}\n"));
}
