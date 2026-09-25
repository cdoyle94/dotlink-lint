use std::collections::HashMap;
use std::fmt;
use std::path::{Component, Path};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Finding {
    pub line: usize,
    pub severity: Severity,
    pub message: String,
}

impl Finding {
    fn error(line: usize, message: String) -> Finding {
        Finding { line, severity: Severity::Error, message }
    }

    fn warning(line: usize, message: String) -> Finding {
        Finding { line, severity: Severity::Warning, message }
    }
}

struct Entry {
    line: usize,
    source: String,
    target: String,
}

/// Parses a manifest of lines like `zshrc/zshrc -> ~/.zshrc`.
/// Blank lines and lines starting with `#` (after trimming) are ignored.
fn parse(text: &str) -> (Vec<Entry>, Vec<Finding>) {
    let mut entries = Vec::new();
    let mut findings = Vec::new();

    for (i, raw_line) in text.lines().enumerate() {
        let line_no = i + 1;
        let trimmed = raw_line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // split_once takes the first "->", so a target that itself contains
        // "->" ends up folded into the target half rather than rejected.
        // That's deliberate: the format has no quoting, so we pick the
        // reading that keeps parsing total instead of guessing at intent.
        match trimmed.split_once("->") {
            None => {
                findings.push(Finding::error(
                    line_no,
                    "expected 'source -> target', no '->' found".to_string(),
                ));
            }
            Some((raw_source, raw_target)) => {
                let source = raw_source.trim();
                let target = raw_target.trim();

                if source.is_empty() {
                    findings.push(Finding::error(line_no, "source path is empty".to_string()));
                } else if target.is_empty() {
                    findings.push(Finding::error(line_no, "target path is empty".to_string()));
                } else {
                    entries.push(Entry {
                        line: line_no,
                        source: source.to_string(),
                        target: target.to_string(),
                    });
                }
            }
        }
    }

    (entries, findings)
}

fn escapes_repo(source: &str) -> bool {
    Path::new(source)
        .components()
        .any(|c| c == Component::ParentDir)
}

fn looks_rooted(target: &str) -> bool {
    target.starts_with("~/") || target.starts_with('/')
}

fn check_entries(entries: &[Entry]) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut seen_sources: HashMap<&str, usize> = HashMap::new();
    let mut seen_targets: HashMap<&str, usize> = HashMap::new();

    for entry in entries {
        if let Some(&first_line) = seen_sources.get(entry.source.as_str()) {
            findings.push(Finding::error(
                entry.line,
                format!(
                    "source '{}' is already linked at line {}",
                    entry.source, first_line
                ),
            ));
        } else {
            seen_sources.insert(&entry.source, entry.line);
        }

        // Two sources landing on the same target is the dangerous case: the
        // second link silently overwrites the first with no error from `ln`.
        if let Some(&first_line) = seen_targets.get(entry.target.as_str()) {
            findings.push(Finding::error(
                entry.line,
                format!(
                    "target '{}' is already claimed at line {} and would be overwritten",
                    entry.target, first_line
                ),
            ));
        } else {
            seen_targets.insert(&entry.target, entry.line);
        }

        if escapes_repo(&entry.source) {
            findings.push(Finding::error(
                entry.line,
                format!("source '{}' escapes the dotfiles repo via '..'", entry.source),
            ));
        }

        if !looks_rooted(&entry.target) {
            findings.push(Finding::warning(
                entry.line,
                format!(
                    "target '{}' should start with '~/' or '/' so it resolves the same way regardless of cwd",
                    entry.target
                ),
            ));
        }
    }

    findings
}

// Missing sources are only worth reporting once. If a source already escapes
// the repo, check_entries has flagged that; piling "also doesn't exist" on
// top of a path that shouldn't be there in the first place is just noise.
fn check_sources_exist(entries: &[Entry], root: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();

    for entry in entries {
        if escapes_repo(&entry.source) {
            continue;
        }

        if !root.join(&entry.source).exists() {
            findings.push(Finding::error(
                entry.line,
                format!(
                    "source '{}' does not exist under {}",
                    entry.source,
                    root.display()
                ),
            ));
        }
    }

    findings
}

pub fn lint(text: &str) -> Vec<Finding> {
    let (entries, mut findings) = parse(text);
    findings.extend(check_entries(&entries));
    findings.sort_by_key(|f| f.line);
    findings
}

/// Same static checks as `lint`, plus a filesystem check that each source
/// actually exists under `root` (the directory the checks treat as the
/// dotfiles repo root).
pub fn lint_with_root(text: &str, root: &Path) -> Vec<Finding> {
    let (entries, mut findings) = parse(text);
    findings.extend(check_entries(&entries));
    findings.extend(check_sources_exist(&entries, root));
    findings.sort_by_key(|f| f.line);
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Case {
        name: &'static str,
        manifest: &'static str,
        // (line, severity, substring expected in the message)
        expected: &'static [(usize, Severity, &'static str)],
    }

    #[test]
    fn table_driven_cases() {
        let cases: &[Case] = &[
            Case {
                name: "empty file has no findings",
                manifest: "",
                expected: &[],
            },
            Case {
                name: "comments and blank lines are ignored",
                manifest: "# top of file\n\n   # indented comment\n\n",
                expected: &[],
            },
            Case {
                name: "a well formed entry is clean",
                manifest: "vimrc/vimrc -> ~/.vimrc\n",
                expected: &[],
            },
            Case {
                name: "surrounding whitespace around the arrow is tolerated",
                manifest: "vimrc/vimrc\t->\t~/.vimrc\n",
                expected: &[],
            },
            Case {
                name: "missing arrow is an error",
                manifest: "vimrc/vimrc ~/.vimrc\n",
                expected: &[(1, Severity::Error, "no '->' found")],
            },
            Case {
                name: "empty source is an error",
                manifest: " -> ~/.vimrc\n",
                expected: &[(1, Severity::Error, "source path is empty")],
            },
            Case {
                name: "empty target is an error",
                manifest: "vimrc/vimrc ->   \n",
                expected: &[(1, Severity::Error, "target path is empty")],
            },
            Case {
                name: "duplicate source is an error on the second line",
                manifest: "vimrc/vimrc -> ~/.vimrc\nvimrc/vimrc -> ~/.vimrc2\n",
                expected: &[(2, Severity::Error, "already linked at line 1")],
            },
            Case {
                name: "duplicate target is an error even with different sources",
                manifest: "vimrc/vimrc -> ~/.vimrc\nvimrc/other -> ~/.vimrc\n",
                expected: &[(2, Severity::Error, "would be overwritten")],
            },
            Case {
                name: "source escaping the repo via .. is an error",
                manifest: "../../etc/passwd -> ~/.passwd\n",
                expected: &[(1, Severity::Error, "escapes the dotfiles repo")],
            },
            Case {
                name: "a relative-looking target is a warning, not an error",
                manifest: "vimrc/vimrc -> .vimrc\n",
                expected: &[(1, Severity::Warning, "should start with '~/' or '/'")],
            },
            Case {
                name: "a target that itself contains an arrow still parses, just gets flagged",
                manifest: "a -> b -> c\n",
                expected: &[(1, Severity::Warning, "should start with '~/' or '/'")],
            },
            Case {
                name: "line numbers stay correct after earlier blank and comment lines",
                manifest: "# header\n\nvimrc/vimrc ~/.vimrc\n",
                expected: &[(3, Severity::Error, "no '->' found")],
            },
        ];

        for case in cases {
            let findings = lint(case.manifest);
            assert_eq!(
                findings.len(),
                case.expected.len(),
                "case '{}': expected {} finding(s), got {:?}",
                case.name,
                case.expected.len(),
                findings
            );
            for (finding, (line, severity, substring)) in findings.iter().zip(case.expected.iter()) {
                assert_eq!(finding.line, *line, "case '{}': wrong line number", case.name);
                assert_eq!(finding.severity, *severity, "case '{}': wrong severity", case.name);
                assert!(
                    finding.message.contains(substring),
                    "case '{}': message '{}' did not contain '{}'",
                    case.name,
                    finding.message,
                    substring
                );
            }
        }
    }

    #[test]
    fn missing_source_is_flagged_against_the_filesystem() {
        let root = std::env::temp_dir().join(format!(
            "dotlink-lint-test-{}-{}",
            std::process::id(),
            "missing_source_is_flagged_against_the_filesystem"
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("present"), b"").unwrap();

        let manifest = "present -> ~/.present\nmissing -> ~/.missing\n";
        let findings = lint_with_root(manifest, &root);

        assert_eq!(findings.len(), 1, "expected one finding, got {:?}", findings);
        assert_eq!(findings[0].line, 2);
        assert_eq!(findings[0].severity, Severity::Error);
        assert!(findings[0].message.contains("does not exist"));

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn missing_source_that_also_escapes_the_repo_is_only_flagged_once() {
        let root = std::env::temp_dir().join(format!(
            "dotlink-lint-test-{}-{}",
            std::process::id(),
            "missing_source_that_also_escapes_the_repo_is_only_flagged_once"
        ));
        std::fs::create_dir_all(&root).unwrap();

        let manifest = "../../etc/passwd -> ~/.passwd\n";
        let findings = lint_with_root(manifest, &root);

        assert_eq!(findings.len(), 1, "expected one finding, got {:?}", findings);
        assert!(findings[0].message.contains("escapes the dotfiles repo"));

        std::fs::remove_dir_all(&root).unwrap();
    }
}
