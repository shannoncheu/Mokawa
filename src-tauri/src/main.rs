#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[cfg(windows)]
mod app;
fn main() {
    #[cfg(windows)]
    app::run();
    #[cfg(not(windows))]
    eprintln!("Mokawa 仅支持 Windows 10/11 x64；本环境仅验证 Rust 核心。");
}
