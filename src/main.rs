use clap::Parser;
use std::process::ExitCode;
fn main() -> ExitCode {
    let cli = match xcli::cli::Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                let _ = e.print();
                return ExitCode::SUCCESS;
            }
            // Clap errors may echo argv, so emit only a static machine-readable usage error.
            eprintln!(
                "{{\"kind\":\"invalid_input\",\"message\":\"Invalid arguments; see xcli --help\",\"retry_after_seconds\":null}}"
            );
            return ExitCode::from(2);
        }
    };
    let result = xcli::transport::Http::new().and_then(|t| {
        xcli::app::execute(&cli, &t, &xcli::app::SystemCredentials, xcli::app::discover)
    });
    match result {
        Ok(out) => {
            if cli.human {
                println!("{}", xcli::app::human(&out));
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&out).expect("serializable JSON")
                );
            }
            if out.get("complete") == Some(&serde_json::Value::Bool(false)) {
                ExitCode::from(12)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("{}", serde_json::to_string(&e).expect("serializable error"));
            ExitCode::from(e.exit_code())
        }
    }
}
