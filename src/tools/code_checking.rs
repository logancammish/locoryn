use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

const MAX_CODE_BYTES: usize = 512 * 1024;
const MAX_OUTPUT_CHARS: usize = 4_000;
const MAX_CAPTURED_OUTPUT_BYTES: usize = 16 * 1024;
const CHECK_TIMEOUT: Duration = Duration::from_secs(6);
static TEMP_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn canonical_language(language: &str) -> Option<&'static str> {
    match language.trim().to_ascii_lowercase().as_str() {
        "python" | "py" | "python3" => Some("Python"),
        "rust" | "rs" => Some("Rust"),
        "c" => Some("C"),
        "cpp" | "c++" | "cxx" | "cplusplus" => Some("C++"),
        "cs" | "c#" | "csharp" | "c-sharp" => Some("C#"),
        _ => None,
    }
}

pub(crate) fn tool_definition() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "check_code",
            "description": "Check whether a self-contained Python, Rust, C, C++, or C# code snippet compiles or has syntax errors. Use this to verify code you propose when compilation feedback would help. The checker never runs the snippet, has a short time limit, and rejects snippets that try to load extra local files.",
            "parameters": {
                "type": "object",
                "required": ["language", "code"],
                "properties": {
                    "language": {
                        "type": "string",
                        "enum": ["python", "rust", "c", "cpp", "csharp"],
                        "description": "The snippet language."
                    },
                    "code": {
                        "type": "string",
                        "description": "The complete, self-contained snippet to check."
                    }
                }
            }
        }
    })
}

pub(crate) fn check_code(language: &str, code: &str) -> Result<String, String> {
    let language = canonical_language(language)
        .ok_or_else(|| "Code checking supports Python, Rust, C, C++, and C#.".to_string())?;
    if code.len() > MAX_CODE_BYTES {
        return Err(format!(
            "Code snippets must be at most {MAX_CODE_BYTES} bytes to check safely."
        ));
    }
    reject_external_source_includes(language, code)?;

    let directory = create_check_directory()?;
    let result = check_in_directory(language, code, &directory);
    let _ = fs::remove_dir_all(&directory);
    result
}

fn check_in_directory(language: &str, code: &str, directory: &Path) -> Result<String, String> {
    let (file_name, mut command, arguments): (&str, Command, Vec<&str>) = match language {
        "Python" => (
            "snippet.py",
            Command::new("python3"),
            vec!["-m", "py_compile", "snippet.py"],
        ),
        "Rust" => (
            "snippet.rs",
            Command::new("rustc"),
            vec![
                "--crate-type",
                "lib",
                "--emit",
                "metadata",
                "snippet.rs",
                "-o",
                "snippet.rmeta",
            ],
        ),
        "C" => (
            "snippet.c",
            Command::new("cc"),
            vec!["-fsyntax-only", "snippet.c"],
        ),
        "C++" => (
            "snippet.cpp",
            Command::new("c++"),
            vec!["-fsyntax-only", "snippet.cpp"],
        ),
        "C#" => (
            "snippet.cs",
            Command::new("csc"),
            vec![
                "/nologo",
                "/target:library",
                "/out:snippet.dll",
                "snippet.cs",
            ],
        ),
        _ => unreachable!("canonical_language returned an unsupported value"),
    };

    write_new_file(&directory.join(file_name), code)?;
    command
        .args(arguments)
        .current_dir(directory)
        // Generated code must not inherit user-controlled compiler/interpreter
        // options such as PYTHONPATH, RUSTFLAGS, CFLAGS, or CXXFLAGS.
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Keep executable discovery working after env_clear without restoring any
    // language-specific compiler/interpreter configuration variables.
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    let output = run_with_timeout(command, language)?;
    let details = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .trim()
    .chars()
    .take(MAX_OUTPUT_CHARS)
    .collect::<String>();
    if output.status.success() {
        Ok(if details.is_empty() {
            format!("{language} check passed with no errors.")
        } else {
            format!("{language} check passed:\n{details}")
        })
    } else {
        Err(format!(
            "{language} check found errors{}{}",
            if details.is_empty() { "." } else { ":\n" },
            details
        ))
    }
}

fn run_with_timeout(mut command: Command, language: &str) -> Result<std::process::Output, String> {
    let mut child = command.spawn().map_err(|error| {
        format!("{language} checker is unavailable. Install its compiler/interpreter and try again: {error}")
    })?;
    let stdout = child.stdout.take().map(capture_output);
    let stderr = child.stderr.take().map(capture_output);
    let started = Instant::now();
    loop {
        if child
            .try_wait()
            .map_err(|error| format!("Could not wait for the {language} checker: {error}"))?
            .is_some()
        {
            return collect_output(child, stdout, stderr, language);
        }
        if started.elapsed() >= CHECK_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            // Do not wait on reader threads here: a compiler descendant could
            // keep an inherited pipe open after the direct child is killed.
            // Dropping their handles lets this call honour its time bound.
            drop(stdout);
            drop(stderr);
            return Err(format!(
                "{language} check timed out after {} seconds.",
                CHECK_TIMEOUT.as_secs()
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn capture_output<R: Read + Send + 'static>(mut reader: R) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut captured = Vec::with_capacity(MAX_CAPTURED_OUTPUT_BYTES);
        let mut buffer = [0_u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    let remaining = MAX_CAPTURED_OUTPUT_BYTES.saturating_sub(captured.len());
                    captured.extend_from_slice(&buffer[..read.min(remaining)]);
                }
            }
        }
        captured
    })
}

fn collect_output(
    child: std::process::Child,
    stdout: Option<thread::JoinHandle<Vec<u8>>>,
    stderr: Option<thread::JoinHandle<Vec<u8>>>,
    language: &str,
) -> Result<std::process::Output, String> {
    let status = child
        .wait_with_output()
        .map_err(|error| format!("Could not collect {language} checker output: {error}"))?
        .status;
    Ok(std::process::Output {
        status,
        stdout: join_output(stdout),
        stderr: join_output(stderr),
    })
}

fn join_output(handle: Option<thread::JoinHandle<Vec<u8>>>) -> Vec<u8> {
    handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default()
}

fn create_check_directory() -> Result<PathBuf, String> {
    for _ in 0..64 {
        let unique = TEMP_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "locoryn-code-check-{}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(),
            unique,
        ));
        match fs::create_dir(&directory) {
            Ok(()) => return Ok(directory),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "Could not create a temporary check folder: {error}"
                ));
            }
        }
    }
    Err("Could not create a unique temporary check folder.".to_string())
}

fn write_new_file(path: &Path, code: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("Could not prepare the code check: {error}"))?;
    file.write_all(code.as_bytes())
        .map_err(|error| format!("Could not prepare the code check: {error}"))
}

fn reject_external_source_includes(language: &str, code: &str) -> Result<(), String> {
    let loads_external_source = match language {
        "Rust" => code.lines().any(|line| {
            let compact = line
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect::<String>();
            compact.contains("include!")
                || compact.contains("include_str!")
                || compact.contains("include_bytes!")
                || compact.contains("#[path")
        }),
        "C" | "C++" => code.lines().any(|line| {
            let trimmed = line.trim_start();
            let Some(after_hash) = trimmed.strip_prefix('#') else {
                return false;
            };
            let Some(after_include) = after_hash.trim_start().strip_prefix("include") else {
                return false;
            };
            // System headers are normal for C/C++ snippets. Quoted and
            // macro-based includes can resolve to user-controlled files;
            // absolute and parent-relative angle includes can as well.
            let header = after_include.trim_start();
            !header.starts_with('<') || header.starts_with("</") || header.contains("..")
        }),
        "C#" => code.lines().any(|line| {
            line.chars()
                .filter(|character| !character.is_whitespace())
                .collect::<String>()
                .starts_with("#load")
        }),
        "Python" => false,
        _ => unreachable!("canonical_language returned an unsupported value"),
    };
    if loads_external_source {
        return Err(
            "Code checking accepts self-contained snippets only; loading additional local source files is not allowed."
                .to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{canonical_language, check_code, reject_external_source_includes, tool_definition};
    use std::process::Command;

    #[test]
    fn accepts_only_supported_languages() {
        assert_eq!(canonical_language("python3"), Some("Python"));
        assert_eq!(canonical_language("rs"), Some("Rust"));
        assert_eq!(canonical_language("c++"), Some("C++"));
        assert_eq!(canonical_language("csharp"), Some("C#"));
        assert_eq!(canonical_language("javascript"), None);
    }

    #[test]
    fn rejects_source_directives_that_can_read_other_files() {
        assert!(reject_external_source_includes("Rust", "include_str!(\"/etc/passwd\")").is_err());
        assert!(reject_external_source_includes("C", "#include \"../secret.h\"").is_err());
        assert!(reject_external_source_includes("C#", "#load \"secret.csx\"").is_err());
        assert!(reject_external_source_includes("C++", "#include <vector>").is_ok());
        assert!(reject_external_source_includes("C", "#include </etc/passwd>").is_err());
    }

    #[test]
    fn tool_definition_requires_language_and_code() {
        let definition = tool_definition();
        assert_eq!(definition["function"]["name"], "check_code");
        assert_eq!(
            definition["function"]["parameters"]["required"],
            serde_json::json!(["language", "code"])
        );
    }

    #[test]
    fn python_check_compiles_without_executing_the_snippet() {
        if Command::new("python3").arg("--version").output().is_err() {
            return;
        }

        let result = check_code("python", "raise RuntimeError('must not run')");
        assert!(matches!(result, Ok(message) if message.contains("passed")));
    }
}
