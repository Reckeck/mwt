//! The `ci-scripts` check: the mistakes in the CI scripts that otherwise only show up on a runner.
//!
//! - `*.ps1` under scripts/ci must be ASCII: Windows PowerShell 5.1 reads UTF-8 without a BOM as
//!   cp1252, where the bytes of an em dash contain 0x94 and end a string early; and a bare `a,b`
//!   among command arguments is an array, not one argument;
//! - every shell script must parse (`bash -n`) and be executable (CI runs it directly), and pass
//!   shellcheck when it is installed;
//! - every COPY source of scripts/ci/ci.Dockerfile must be let through by .dockerignore (a `*`
//!   plus an allow-list), or the image build finds nothing.
//!
//! (The channel names against the Tauri configs are unit tests of this crate.)

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::exec::which;

const SHELL_DIRS: [(&str, &str); 3] = [
    ("scripts/ci", "sh"),
    ("scripts/local", "sh"),
    ("packaging/linux", "sh"),
];
const SHELLCHECKED: [&str; 1] = ["scripts/ci"];
const RENDER: &str = "packaging/aur/render.sh";

/// The 1-based lines with a non-ASCII character.
pub fn non_ascii_lines(text: &str) -> Vec<usize> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.is_ascii())
        .map(|(index, _)| index + 1)
        .collect()
}

/// A command line (`Invoke-Something ...` or `& ...`) with an unquoted `a,b` word among its
/// arguments: PowerShell passes that as one array.
pub fn has_unquoted_comma_list(line: &str) -> bool {
    let mut words = line.split_whitespace();
    let Some(first) = words.next() else {
        return false;
    };
    let is_command = first == "&"
        || first.strip_prefix("Invoke-").is_some_and(|rest| {
            !rest.is_empty()
                && rest
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        });
    is_command && words.any(is_comma_pair)
}

/// `word,word` with one comma and only `[A-Za-z0-9_.-]` around it (a quoted list starts with a quote).
fn is_comma_pair(word: &str) -> bool {
    let plain = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-'))
    };
    word.split_once(',')
        .is_some_and(|(left, right)| plain(left) && plain(right))
}

/// The first argument of each `COPY` (not `--from=`) line of a Dockerfile.
pub fn copy_sources(dockerfile: &str) -> Vec<String> {
    dockerfile
        .lines()
        .filter(|line| line.starts_with("COPY ") && !line.contains("--from="))
        .filter_map(|line| line.split_whitespace().nth(1))
        .map(String::from)
        .collect()
}

/// A `!<source>` line of .dockerignore, whole-line.
pub fn dockerignore_allows(ignore: &str, source: &str) -> bool {
    let wanted = format!("!{source}");
    ignore.lines().any(|line| line == wanted)
}

fn files_with_extension(root: &Path, dir: &str, extension: &str) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(root.join(dir))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .collect();
    found.sort();
    found
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn check_powershell(root: &Path, problems: &mut Vec<String>) {
    for script in files_with_extension(root, "scripts/ci", "ps1") {
        let name = rel(root, &script);
        let Ok(text) = std::fs::read_to_string(&script) else {
            problems.push(format!("{name}: not readable as UTF-8 text"));
            continue;
        };
        for line in non_ascii_lines(&text) {
            problems.push(format!(
                "{name}:{line}: non-ASCII characters break Windows PowerShell 5.1 - use ASCII"
            ));
        }
        for (index, line) in text.lines().enumerate() {
            if has_unquoted_comma_list(line) {
                problems.push(format!(
                    "{name}:{}: unquoted comma list in a command - quote it",
                    index + 1
                ));
            }
        }
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}

fn shell_scripts(root: &Path) -> Vec<PathBuf> {
    let mut scripts: Vec<PathBuf> = SHELL_DIRS
        .iter()
        .flat_map(|(dir, extension)| files_with_extension(root, dir, extension))
        .collect();
    scripts.push(root.join(RENDER));
    scripts
}

fn check_shell(root: &Path, problems: &mut Vec<String>) {
    for script in shell_scripts(root) {
        let name = rel(root, &script);
        match Command::new("bash").arg("-n").arg(&script).output() {
            Ok(out) if out.status.success() => {}
            Ok(out) => problems.push(format!(
                "{name}: does not parse\n{}",
                String::from_utf8_lossy(&out.stderr).trim_end()
            )),
            Err(error) => problems.push(format!("{name}: cannot run bash: {error}")),
        }
        // CI runs them directly: without the bit a job dies with "Permission denied".
        if !is_executable(&script) {
            problems.push(format!("{name}: not executable, chmod +x it"));
        }
    }
}

fn check_dockerfile(root: &Path, problems: &mut Vec<String>) {
    let dockerfile =
        std::fs::read_to_string(root.join("scripts/ci/ci.Dockerfile")).unwrap_or_default();
    let ignore = std::fs::read_to_string(root.join(".dockerignore")).unwrap_or_default();
    for source in copy_sources(&dockerfile) {
        if !dockerignore_allows(&ignore, &source) {
            problems.push(format!(
                "scripts/ci/ci.Dockerfile copies {source}, but .dockerignore filters it out - add !{source}"
            ));
        }
    }
}

fn check_shellcheck(root: &Path, problems: &mut Vec<String>) {
    if which("shellcheck").is_none() {
        return;
    }
    let mut files: Vec<PathBuf> = SHELLCHECKED
        .iter()
        .flat_map(|dir| files_with_extension(root, dir, "sh"))
        .collect();
    files.push(root.join(RENDER));
    let out = Command::new("shellcheck")
        .args(["-S", "warning", "-x"])
        .args(&files)
        .current_dir(root)
        .output();
    match out {
        Ok(out) if out.status.success() => {}
        Ok(out) => problems.push(format!(
            "shellcheck:\n{}",
            String::from_utf8_lossy(&out.stdout).trim_end()
        )),
        Err(error) => problems.push(format!("cannot run shellcheck: {error}")),
    }
}

/// Every problem found, one entry each; empty when the CI scripts are fine.
pub fn check(root: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    check_powershell(root, &mut problems);
    check_shell(root, &mut problems);
    check_dockerfile(root, &mut problems);
    check_shellcheck(root, &mut problems);
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_ascii_lines_are_found_by_number() {
        assert_eq!(non_ascii_lines("a\nb \u{2014} c\nd\n\u{e9}"), [2, 4]);
        assert!(non_ascii_lines("plain\ntext").is_empty());
    }

    #[test]
    fn a_bare_comma_list_in_a_command_is_flagged() {
        assert!(has_unquoted_comma_list(
            "  Invoke-Build --bundles nsis,msi --x"
        ));
        assert!(has_unquoted_comma_list("& cargo tauri build a.b,c-d"));
        assert!(has_unquoted_comma_list("& tool nsis,msi"));
    }

    #[test]
    fn quoted_lists_and_other_lines_are_fine() {
        assert!(!has_unquoted_comma_list(
            "  Invoke-Build --bundles 'nsis,msi'"
        ));
        assert!(!has_unquoted_comma_list("& tool \"nsis,msi\""));
        assert!(!has_unquoted_comma_list("$x = a,b"));
        assert!(!has_unquoted_comma_list("Invoke-Thing a,b,c"));
        assert!(!has_unquoted_comma_list("Invoke-Thing ,b a,"));
        assert!(!has_unquoted_comma_list("Invoke-Thing"));
        assert!(!has_unquoted_comma_list("&tool a,b"));
    }

    #[test]
    fn copy_sources_skip_from_stages() {
        let text = "FROM x\nCOPY mise.toml /app/\nCOPY --from=a /x /y\nRUN COPY q\nCOPY scripts/ci/a.sh /opt/\n";
        assert_eq!(copy_sources(text), ["mise.toml", "scripts/ci/a.sh"]);
    }

    #[test]
    fn dockerignore_needs_the_exact_allow_line() {
        let ignore = "*\n!mise.toml\n!scripts/ci/apt-packages.sh\n";
        assert!(dockerignore_allows(ignore, "mise.toml"));
        assert!(!dockerignore_allows(ignore, "scripts/ci"));
        assert!(!dockerignore_allows(ignore, "toml"));
    }

    #[test]
    fn the_repository_passes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        assert_eq!(check(&root), Vec::<String>::new());
    }
}
