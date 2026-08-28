// 赣语输入法 — 桌面键盘转发工具（独立二进制入口）
mod forward;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let skip_install = args.iter().any(|a| a == "--skip-install" || a == "-S");
    forward::run(!skip_install)
}
