use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::{env, fs};

fn usage() -> ! {
    eprintln!("Usage: gannyu-sanitize-binary <binary> [--strip-tool <path>] [--no-strip] [--verify-release --repo-root <path>]");
    std::process::exit(2);
}

fn find_strip_tool(explicit: Option<&str>) -> Option<String> {
    if let Some(path) = explicit {
        if std::path::Path::new(path).is_file() {
            return Some(path.to_owned());
        }
        return None;
    }
    for name in &["llvm-objcopy", "llvm-strip", "strip"] {
        let lookup = if cfg!(windows) { "where" } else { "which" };
        if let Ok(output) = Command::new(lookup).arg(name).output() {
            if output.status.success() {
                let found = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                if !found.is_empty() {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn strip_symbols(binary: &str, tool: &str) -> Result<(), String> {
    let status = Command::new(tool)
        .args(["--strip-all", binary])
        .status()
        .map_err(|e| format!("failed to run {tool}: {e}"))?;
    if !status.success() {
        return Err(format!("{tool} --strip-all exited with {status}"));
    }
    Ok(())
}

fn scrub_sensitive_strings(data: &mut [u8]) -> usize {
    let patterns: &[&[u8]] = &[
        b"crates/ffi/src/",
        b"crates/core/src/",
        b"/media/",
        b"/home/",
        b"/Users/",
        b"/rustc/",
        b"/rust/registry/",
    ];

    let mut count = 0;
    for pattern in patterns {
        let plen = pattern.len();
        let mut i = 0;
        while i + plen <= data.len() {
            if &data[i..i + plen] == *pattern {
                let end = data[i..]
                    .iter()
                    .position(|&b| b == 0 || b == b'"' || b == b'\n' || b >= 128)
                    .map(|off| i + off)
                    .unwrap_or_else(|| (i + plen).min(data.len()));
                let chunk = &data[i..end];
                if chunk.iter().all(|&b| (32..127).contains(&b)) {
                    let replacement: Vec<u8> = vec![b'x'; end - i];
                    data[i..end].copy_from_slice(&replacement);
                    count += 1;
                }
                i = end;
            } else {
                i += 1;
            }
        }
    }
    count
}

fn verify_release(data: &[u8], repo_root: Option<&PathBuf>) -> Result<(), String> {
    let patterns: &[(&str, &[u8])] = &[
        ("source path", b"crates/ffi/src/"),
        ("source path", b"crates/core/src/"),
        ("resource path", b"manifest.toml"),
        ("resource path", b"regions/"),
        ("resource path", b"frequency/"),
        ("Rust symbol", b"gannyu_input_core::"),
        ("Rust symbol", b"gannyu_input_ffi::"),
    ];
    let mut leaks: Vec<&str> = patterns
        .iter()
        .filter_map(|(name, pattern)| {
            data.windows(pattern.len())
                .any(|chunk| chunk == *pattern)
                .then_some(*name)
        })
        .collect();
    if let Some(root) = repo_root {
        let root = root.to_string_lossy();
        if !root.is_empty()
            && data
                .windows(root.len())
                .any(|chunk| chunk == root.as_bytes())
        {
            leaks.push("absolute workspace path");
        }
    }
    leaks.sort_unstable();
    leaks.dedup();
    if leaks.is_empty() {
        Ok(())
    } else {
        Err(format!("release artifact contains {}", leaks.join(", ")))
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let mut binary: Option<PathBuf> = None;
    let mut strip_tool: Option<String> = None;
    let mut repo_root: Option<PathBuf> = None;
    let mut no_strip = false;
    let mut verify = false;

    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--strip-tool" => strip_tool = iter.next().map(|s| s.to_owned()),
            "--repo-root" => repo_root = iter.next().map(PathBuf::from),
            "--no-strip" => no_strip = true,
            "--verify-release" => verify = true,
            other => {
                if other.starts_with('-') {
                    eprintln!("unknown flag: {other}");
                    usage();
                }
                binary = Some(PathBuf::from(other));
            }
        }
    }

    let binary = match binary {
        Some(p) if p.is_file() => p,
        Some(p) => {
            eprintln!("error: binary not found: {}", p.display());
            return ExitCode::FAILURE;
        }
        None => usage(),
    };
    let binary_str = binary.to_string_lossy();
    let mut data = match fs::read(&binary) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error reading {binary_str}: {e}");
            return ExitCode::FAILURE;
        }
    };

    if verify {
        return match verify_release(&data, repo_root.as_ref()) {
            Ok(()) => {
                eprintln!("[sanitize] release artifact scan passed");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        };
    }

    if !no_strip {
        match find_strip_tool(strip_tool.as_deref()) {
            Some(ref tool) => match strip_symbols(&binary_str, tool) {
                Ok(()) => eprintln!("[sanitize] stripped symbols with {tool}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::FAILURE;
                }
            },
            None => eprintln!("warning: no strip tool found; skipping symbol stripping"),
        }
    }

    let count = scrub_sensitive_strings(&mut data);
    if let Err(e) = fs::write(&binary, &data) {
        eprintln!("error writing {binary_str}: {e}");
        return ExitCode::FAILURE;
    }
    eprintln!("[sanitize] scrubbed {count} sensitive string(s)");
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::verify_release;
    use std::path::PathBuf;

    #[test]
    fn release_scan_accepts_clean_data() {
        assert!(verify_release(b"release artifact", None).is_ok());
    }

    #[test]
    fn release_scan_rejects_sensitive_data() {
        let error = verify_release(
            b"crates/core/src/lib.rs",
            Some(&PathBuf::from("/workspace")),
        )
        .unwrap_err();
        assert!(error.contains("source path"));
    }
}
