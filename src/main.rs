mod linter;

use std::env;
use std::fs;
use std::path::Path;
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

    // Sources are resolved relative to the manifest's own directory: that's
    // where a dotfiles repo's manifest normally lives, and it's the closest
    // thing to a repo root until there's real config for one (see README).
    let root = Path::new(&path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let findings = linter::lint_with_root(&text, root);
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
