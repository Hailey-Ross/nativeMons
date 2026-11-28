//! The launcher's platform layer, apart from nm-ui: the window (SDL3, feature `sdl`), input in the C# launcher's
//! terms, one launcher per folder with requests, and Light mode's detection.

/// NATIVEMONS_TEST_QUIET=1 (the C#'s Native.Quiet): a test's launcher never takes the foreground.
pub fn quiet() -> bool { std::env::var("NATIVEMONS_TEST_QUIET").as_deref() == Ok("1") }

pub mod geometry;
pub mod input;
pub mod dialogs;
pub mod instance;
pub mod light;
pub mod process;
pub mod snaps;
#[cfg(any(unix, test))]
pub mod x11;
#[cfg(feature = "sdl")]
pub mod window;
