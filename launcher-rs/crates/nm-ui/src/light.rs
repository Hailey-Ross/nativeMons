//! Light mode (Rust only, crossplatform note section 8): Auto, On or Off, kept in state\light-mode.txt.
//! The look stays the same; only rates change (animations at 30 fps, no idle full-rate redraws).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightSetting { Auto, On, Off }

pub fn read() -> LightSetting {
    match nm_core::config::read("light-mode.txt", "auto").trim().to_ascii_lowercase().as_str() {
        "on" => LightSetting::On,
        "off" => LightSetting::Off,
        _ => LightSetting::Auto,
    }
}
pub fn write(s: LightSetting) { nm_core::config::write("light-mode.txt", match s { LightSetting::Auto => "auto", LightSetting::On => "on", LightSetting::Off => "off" }); }
/// Selecting the row steps Auto -> On -> Off -> Auto.
pub fn next(s: LightSetting) -> LightSetting { match s { LightSetting::Auto => LightSetting::On, LightSetting::On => LightSetting::Off, LightSetting::Off => LightSetting::Auto } }
pub fn effective(s: LightSetting, auto: bool) -> bool { match s { LightSetting::Auto => auto, LightSetting::On => true, LightSetting::Off => false } }
pub fn row_text(s: LightSetting, auto: bool) -> String {
    match s { LightSetting::Auto => format!("Light mode: Auto ({})", if auto { "on" } else { "off" }), LightSetting::On => "Light mode: On".into(), LightSetting::Off => "Light mode: Off".into() }
}
pub fn row_info(s: LightSetting, auto: bool, why: &str) -> String {
    let chose = format!("Auto turns it {} on this PC: {}.", if auto { "on" } else { "off" }, why);
    let next = match s { LightSetting::Auto => "On", LightSetting::On => "Off", LightSetting::Off => "Auto" };
    format!("Lighter on slower PCs and handhelds: animations at 30 frames a second, no redrawing while nothing moves, Quick start off unless you turn it on, and pictures from your games only when asked. The look stays the same. {} Select for {}.", chose, next)
}
