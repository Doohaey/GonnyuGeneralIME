// 桌面键盘转发模块
// 检测安卓设备，自动安装 APK，将电脑键盘输入通过 Gannyu 引擎在桌面端解析候选，
// 用户选择后通过 adb 将最终中文文本发送到安卓设备。
use std::env;
use std::io::{self, IsTerminal, Write};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use termion::input::TermRead;
use termion::raw::IntoRawMode;

const IME_ID: &str = "io.gannyu.input/.GannyuInputMethodService";
const BUILD_SCRIPT: &str = "platforms/android/build_and_install.sh";

/// 运行键盘转发。`auto_install` 为 true 时自动检测设备 → 构建安装 APK → 启用输入法。
pub fn run(auto_install: bool) -> std::process::ExitCode {
    let adb = match find_adb() {
        Some(path) => path,
        None => {
            eprintln!("ERROR: adb not found. Install Android platform-tools.");
            return std::process::ExitCode::FAILURE;
        }
    };

    let devices = match list_devices(&adb) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ERROR: cannot list adb devices: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    if devices.is_empty() {
        eprintln!("ERROR: no Android device connected. Connect via USB and enable USB debugging.");
        return std::process::ExitCode::FAILURE;
    }

    println!("Device: {}", devices[0]);

    if auto_install {
        if !check_ime_installed(&adb) {
            println!("Gannyu IME not installed. Building and installing...");
            match run_build_install() {
                Ok(()) => {
                    println!("APK installed, IME enabled.");
                    thread::sleep(Duration::from_secs(1));
                }
                Err(e) => {
                    eprintln!("Build/install failed: {e}");
                    eprintln!("Run manually: bash {BUILD_SCRIPT}");
                    return std::process::ExitCode::FAILURE;
                }
            }
        } else {
            println!("Gannyu IME is active.");
        }
    }

    println!();
    println!("=== Gannyu Keyboard Forward ===");
    println!("Type pinyin on your keyboard. Press Space to see candidates.");
    println!("Press 1-9 to select a candidate. Press Enter to commit raw text.");
    println!("Press Ctrl+C to exit.");
    println!();

    // Start adb shell session for sending text
    let mut shell = match start_adb_shell(&adb) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ERROR: cannot start adb shell: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let stdin_pipe = match shell.stdin.as_mut() {
        Some(s) => s,
        None => {
            eprintln!("ERROR: adb shell stdin unavailable");
            return std::process::ExitCode::FAILURE;
        }
    };

    let mut composing = String::new();
    let mut candidates: Vec<String> = Vec::new();

    if io::stdin().is_terminal() {
        let _stdout = io::stdout().into_raw_mode().ok();
        let keys = io::stdin();

        for key_result in keys.keys() {
            let key = match key_result {
                Ok(k) => k,
                Err(_) => break,
            };

            let should_exit = match &key {
                termion::event::Key::Ctrl('c') => {
                    println!("\r\nStopped.");
                    true
                }
                termion::event::Key::Char(c @ '1'..='9') => {
                    let idx = (*c as usize) - ('1' as usize);
                    if idx < candidates.len() {
                        let text = &candidates[idx];
                        let escaped = text.replace('\'', "'\\''");
                        writeln!(stdin_pipe, "input text '{}'", escaped).ok();
                        stdin_pipe.flush().ok();
                        print!("\r\n-> {}\r\n", text);
                        composing.clear();
                        candidates.clear();
                        print_prompt(&composing);
                    }
                    false
                }
                termion::event::Key::Char(' ') => {
                    if !composing.is_empty() {
                        candidates = retrieve_candidates(&composing);
                        if candidates.is_empty() {
                            write!(stdin_pipe, "input text '{}'", composing).ok();
                            stdin_pipe.flush().ok();
                            print!("\r\n-> {}\r\n", composing);
                            composing.clear();
                        } else {
                            display_candidates(&candidates);
                        }
                        print_prompt(&composing);
                    }
                    false
                }
                termion::event::Key::Char('\n') | termion::event::Key::Char('\r') => {
                    if !composing.is_empty() {
                        let escaped = composing.replace('\'', "'\\''");
                        writeln!(stdin_pipe, "input text '{}'", escaped).ok();
                        stdin_pipe.flush().ok();
                        print!("\r\n-> {}\r\n", composing);
                        composing.clear();
                        candidates.clear();
                    }
                    false
                }
                termion::event::Key::Backspace => {
                    if !composing.is_empty() {
                        composing.pop();
                        candidates.clear();
                        print_prompt(&composing);
                    }
                    false
                }
                termion::event::Key::Esc => {
                    composing.clear();
                    candidates.clear();
                    print_prompt(&composing);
                    false
                }
                termion::event::Key::Char(c @ 'a'..='z') => {
                    composing.push(*c);
                    candidates.clear();
                    print_prompt(&composing);
                    false
                }
                termion::event::Key::Char('\'') => {
                    composing.push('\'');
                    candidates.clear();
                    print_prompt(&composing);
                    false
                }
                termion::event::Key::Char(',') => {
                    finish_compose(&mut composing, &candidates, stdin_pipe);
                    writeln!(stdin_pipe, "input text '\u{FF0C}'").ok();
                    stdin_pipe.flush().ok();
                    print_prompt(&composing);
                    false
                }
                termion::event::Key::Char('.') => {
                    finish_compose(&mut composing, &candidates, stdin_pipe);
                    writeln!(stdin_pipe, "input text '\u{3002}'").ok();
                    stdin_pipe.flush().ok();
                    print_prompt(&composing);
                    false
                }
                termion::event::Key::Char('?') => {
                    finish_compose(&mut composing, &candidates, stdin_pipe);
                    writeln!(stdin_pipe, "input text '\u{FF1F}'").ok();
                    stdin_pipe.flush().ok();
                    print_prompt(&composing);
                    false
                }
                termion::event::Key::Char('!') => {
                    finish_compose(&mut composing, &candidates, stdin_pipe);
                    writeln!(stdin_pipe, "input text '\u{FF01}'").ok();
                    stdin_pipe.flush().ok();
                    print_prompt(&composing);
                    false
                }
                termion::event::Key::Char(':') => {
                    finish_compose(&mut composing, &candidates, stdin_pipe);
                    writeln!(stdin_pipe, "input text '\u{FF1A}'").ok();
                    stdin_pipe.flush().ok();
                    print_prompt(&composing);
                    false
                }
                termion::event::Key::Char(';') => {
                    finish_compose(&mut composing, &candidates, stdin_pipe);
                    writeln!(stdin_pipe, "input text '\u{FF1B}'").ok();
                    stdin_pipe.flush().ok();
                    print_prompt(&composing);
                    false
                }
                termion::event::Key::Char('(') => {
                    finish_compose(&mut composing, &candidates, stdin_pipe);
                    writeln!(stdin_pipe, "input text '\u{FF08}'").ok();
                    stdin_pipe.flush().ok();
                    print_prompt(&composing);
                    false
                }
                termion::event::Key::Char(')') => {
                    finish_compose(&mut composing, &candidates, stdin_pipe);
                    writeln!(stdin_pipe, "input text '\u{FF09}'").ok();
                    stdin_pipe.flush().ok();
                    print_prompt(&composing);
                    false
                }
                termion::event::Key::Char(c @ ('0'..='9')) if composing.is_empty() => {
                    writeln!(stdin_pipe, "input text '{}'", c).ok();
                    stdin_pipe.flush().ok();
                    false
                }
                _ => false,
            };

            if should_exit {
                break;
            }
        }
    } else {
        println!("Non-interactive stdin detected: type pinyin and press Enter to retrieve or commit. Type a number 1-9 to select a candidate from the last shown list. Type 'quit' to exit.");
        let stdin = io::stdin();
        let mut line = String::new();
        loop {
            line.clear();
            if stdin.read_line(&mut line).is_err() {
                break;
            }
            if line.is_empty() {
                break;
            }
            let input = line.trim_end_matches(&['\r', '\n'][..]).to_string();
            if input.is_empty() {
                continue;
            }
            if input == "quit" {
                break;
            }
            if input.len() == 1 && input.chars().all(|c| c.is_ascii_digit()) {
                let digit = input.chars().next().unwrap();
                if ('1'..='9').contains(&digit) {
                    let idx = (digit as usize) - ('1' as usize);
                    if idx < candidates.len() {
                        let text = &candidates[idx];
                        let escaped = text.replace('\'', "'\\''");
                        writeln!(stdin_pipe, "input text '{}'", escaped).ok();
                        stdin_pipe.flush().ok();
                        print!("\r\n-> {}\r\n", text);
                        composing.clear();
                        candidates.clear();
                    }
                    continue;
                }
            }
            let trimmed = input.trim();
            candidates = retrieve_candidates(trimmed);
            if candidates.is_empty() {
                let escaped = trimmed.replace('\'', "'\\''");
                writeln!(stdin_pipe, "input text '{}'", escaped).ok();
                stdin_pipe.flush().ok();
                print!("\r\n-> {}\r\n", trimmed);
                composing.clear();
                candidates.clear();
            } else {
                display_candidates(&candidates);
                print_prompt(trimmed);
            }
        }
    }

    let _ = stdin_pipe;
    let _ = shell.kill();
    std::process::ExitCode::SUCCESS
}

fn finish_compose(composing: &mut String, candidates: &[String], stdin_pipe: &mut dyn Write) {
    if !composing.is_empty() && !candidates.is_empty() {
        // Commit first candidate before punctuation
        let text = &candidates[0];
        let escaped = text.replace('\'', "'\\''");
        writeln!(stdin_pipe, "input text '{}'", escaped).ok();
        stdin_pipe.flush().ok();
        print!("\r\n-> {}\r\n", text);
    }
    composing.clear();
}

fn retrieve_candidates(input: &str) -> Vec<String> {
    // Call Gannyu CLI to get candidates
    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "gannyu-input-cli",
            "--",
            "pipeline",
            "retrieve",
            input,
        ])
        .current_dir(find_repo_root_ok().unwrap_or_else(|| ".".into()))
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            text.lines()
                .take(9)
                .map(|line| {
                    // Extract just the text part (before tab)
                    line.split('\t').next().unwrap_or(line).to_string()
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn print_prompt(composing: &str) {
    print!("\r> {}\x1b[K", composing);
    io::stdout().flush().ok();
}

fn display_candidates(candidates: &[String]) {
    print!("\r\n");
    for (i, c) in candidates.iter().enumerate() {
        print!("[{}]{}  ", i + 1, c);
    }
    print!("\r\n");
    io::stdout().flush().ok();
}

fn find_repo_root_ok() -> Option<String> {
    let mut cwd = env::current_dir().ok()?;
    for _ in 0..10 {
        if cwd.join("Cargo.toml").exists() && cwd.join("platforms").is_dir() {
            return Some(cwd.to_string_lossy().to_string());
        }
        if !cwd.pop() {
            break;
        }
    }
    None
}

// ── Helper functions below ──

fn find_adb() -> Option<String> {
    if let Ok(output) = Command::new("which").arg("adb").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }
    for root in [
        env::var("ANDROID_SDK_ROOT").ok(),
        env::var("ANDROID_HOME").ok(),
        env::var("HOME").ok().map(|h| format!("{}/Android/Sdk", h)),
        env::var("HOME")
            .ok()
            .map(|h| format!("{}/Library/Android/sdk", h)),
    ]
    .iter()
    .flatten()
    {
        let adb_path = format!("{}/platform-tools/adb", root);
        if std::path::Path::new(&adb_path).exists() {
            return Some(adb_path);
        }
    }
    None
}

fn list_devices(adb: &str) -> Result<Vec<String>, io::Error> {
    let output = Command::new(adb).arg("devices").output()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();
    for line in text.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[1] == "device" {
            devices.push(parts[0].to_string());
        }
    }
    Ok(devices)
}

fn check_ime_installed(adb: &str) -> bool {
    match Command::new(adb)
        .args(["shell", "ime", "list", "-s"])
        .output()
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout).contains(IME_ID),
        Err(_) => false,
    }
}

fn run_build_install() -> Result<(), String> {
    let repo_root = find_repo_root()?;
    let script_path = format!("{}/{}", repo_root, BUILD_SCRIPT);
    if !std::path::Path::new(&script_path).exists() {
        return Err(format!("Build script not found: {script_path}"));
    }
    let status = Command::new("bash")
        .arg(&script_path)
        .current_dir(&repo_root)
        .status()
        .map_err(|e| format!("Cannot run build script: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Build script exit code: {}",
            status.code().unwrap_or(-1)
        ))
    }
}

fn find_repo_root() -> Result<String, String> {
    let mut cwd = env::current_dir().map_err(|e| format!("Cannot get current dir: {e}"))?;
    for _ in 0..10 {
        if cwd.join("Cargo.toml").exists() && cwd.join("platforms").is_dir() {
            return Ok(cwd.to_string_lossy().to_string());
        }
        if !cwd.pop() {
            break;
        }
    }
    Err("Cannot find repository root. Run from the repository directory.".to_string())
}

fn start_adb_shell(adb: &str) -> Result<Child, io::Error> {
    Command::new(adb)
        .arg("shell")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}
