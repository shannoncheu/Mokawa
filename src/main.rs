#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#[cfg(windows)]
mod ui;
fn main() {
    mokawa::settings::init_logging();
    if !cfg!(windows) {
        eprintln!("Mokawa ships only for Windows 10/11 x64.");
    }
    #[cfg(windows)]
    ui::run();
}
