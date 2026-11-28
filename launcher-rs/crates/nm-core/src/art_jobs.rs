//! GameArt's background gate (the C#'s Capturing, Hold, Eager, OnCaptureThread, Watch, StopNow, Background): one
//! capture at a time, behind everything else; its engine is stopped at once when the player needs the machine.

use crate::proc::StartInfo;
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::SeqCst};
use std::sync::Mutex;

static CAPTURING: AtomicBool = AtomicBool::new(false);
static HOLD: AtomicBool = AtomicBool::new(false);
static EAGER: AtomicBool = AtomicBool::new(false);
static CURRENT: AtomicU32 = AtomicU32::new(0);
static GATE: Mutex<()> = Mutex::new(());
thread_local! { static ON_THREAD: Cell<bool> = const { Cell::new(false) }; }

pub fn capturing() -> bool { CAPTURING.load(SeqCst) }
pub fn hold() -> bool { HOLD.load(SeqCst) }
/// At the start, behind the splash: BelowNormal, since nothing else is happening yet.
pub fn eager() -> bool { EAGER.load(SeqCst) }
pub fn set_eager(on: bool) { EAGER.store(on, SeqCst); }
pub fn on_capture_thread() -> bool { ON_THREAD.with(|c| c.get()) }
/// The launcher's own job is running (its `job != null`, set each tick): background captures give way.
static PLAYER_BUSY: AtomicBool = AtomicBool::new(false);
pub fn set_player_busy(on: bool) { PLAYER_BUSY.store(on, SeqCst); }
pub fn player_busy() -> bool { PLAYER_BUSY.load(SeqCst) }

/// Watch: the engine now running; stopped at once if a hold came first.
pub fn watch(pid: u32) { CURRENT.store(pid, SeqCst); if hold() { stop_now(); } }
/// Its run ended: nothing left to stop.
pub fn unwatch(pid: u32) { let _ = CURRENT.compare_exchange(pid, 0, SeqCst, SeqCst); }
/// StopNow: nothing a capture makes is kept unless it finished, so stopping one costs only a retry at the next start.
pub fn stop_now() {
    HOLD.store(true, SeqCst);
    let p = CURRENT.load(SeqCst);
    if p != 0 { crate::proc::kill_pid(p); }
}
/// Background: the work on this thread, one at a time, with the hold lifted.
pub fn background(work: impl FnOnce()) {
    let _g = GATE.lock().unwrap_or_else(|e| e.into_inner());
    HOLD.store(false, SeqCst);
    CAPTURING.store(true, SeqCst);
    ON_THREAD.with(|c| c.set(true));
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work));
    CAPTURING.store(false, SeqCst);
    ON_THREAD.with(|c| c.set(false));
    CURRENT.store(0, SeqCst);
}
/// A capture's engine: BelowNormal when eager, else Idle (on every core).
pub fn lower(psi: &mut StartInfo) {
    if eager() { psi.below_normal = true; return; }
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(64);
    psi.idle = Some(if n == 64 { u64::MAX } else { (1u64 << n) - 1 });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_lifts_the_hold_and_marks_its_thread() {
        stop_now();
        assert!(hold());
        background(|| { assert!(!hold()); assert!(capturing()); assert!(on_capture_thread()); });
        assert!(!capturing());
        assert!(!on_capture_thread());
        watch(0);
        unwatch(0);
    }
}
