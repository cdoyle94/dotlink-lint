mod linter;

use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "dotlink-lint".to_string());

    let path = match args.next() {
        Some(p) => p,
        None => {
            eprintln!("usage: {} <manifest-file>", program);
            return ExitCode::from(2);
        }
    };

    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}: {}", path, e);
            return ExitCode::from(2);
        }
    };

    let findings = linter::lint(&text);
    let mut had_error = false;

    for finding in &findings {
        if finding.severity == linter::Severity::Error {
            had_error = true;
        }
        println!("{}:{}: {}: {}", path, finding.line, finding.severity, finding.message);
    }

    if had_error {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
