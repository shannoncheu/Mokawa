//! One process-wide palette; Mokawa has one main window. No timer or retained Entity.
use gpui::{Rgba, WindowAppearance};
use mokawa::settings::ThemeMode;
use std::sync::atomic::{AtomicBool, Ordering};
static DARK: AtomicBool = AtomicBool::new(false);
pub fn apply(mode: ThemeMode, appearance: WindowAppearance) {
    DARK.store(
        mode.is_dark(matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        )),
        Ordering::Relaxed,
    );
}
pub fn color(dark: u32) -> Rgba {
    if DARK.load(Ordering::Relaxed) {
        return gpui::rgb(dark);
    }
    gpui::rgb(match dark {
        0x141621 => 0xf0f3fa,
        0x191c28 => 0xffffff,
        0x202331 => 0xf1f4f9,
        0x292d40 => 0xe9eef8,
        0x292f42 | 0x3b4058 => 0xdce6f8,
        0x30374f => 0xdce7ff,
        0xe4e7f2 | 0xe2e6ff => 0x202a3e,
        0xaab3ce | 0xa3acc3 | 0x9da6be => 0x526079,
        0x838ba4 | 0x6c758d => 0x61708a,
        0x949ce5 => 0x5966b8,
        0x4a2935 => 0xffe8eb,
        _ => dark,
    })
}
