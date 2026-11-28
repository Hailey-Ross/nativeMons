//! Time spent per drawing stage (cargo feature `prof`; without it every span is empty and free).

#[cfg(feature = "prof")]
mod imp {
    use std::cell::RefCell;
    use std::time::Instant;
    thread_local! { static ACC: RefCell<Vec<(&'static str, f64, u64)>> = const { RefCell::new(Vec::new()) }; }
    pub struct Span(&'static str, Instant);
    impl Drop for Span {
        fn drop(&mut self) {
            let ms = self.1.elapsed().as_secs_f64() * 1000.0;
            ACC.with(|a| {
                let mut a = a.borrow_mut();
                match a.iter_mut().find(|e| e.0 == self.0) { Some(e) => { e.1 += ms; e.2 += 1; } None => a.push((self.0, ms, 1)) }
            });
        }
    }
    pub fn span(name: &'static str) -> Span { Span(name, Instant::now()) }
    pub fn add(name: &'static str, ms: f64) { ACC.with(|a| { let mut a = a.borrow_mut(); match a.iter_mut().find(|e| e.0 == name) { Some(e) => { e.1 += ms; e.2 += 1; } None => a.push((name, ms, 1)) } }); }
    pub fn take() -> Vec<(&'static str, f64, u64)> { ACC.with(|a| std::mem::take(&mut *a.borrow_mut())) }
}
#[cfg(not(feature = "prof"))]
mod imp {
    pub struct Span;
    #[inline(always)]
    pub fn span(_: &'static str) -> Span { Span }
    pub fn add(_: &'static str, _: f64) {}
    pub fn take() -> Vec<(&'static str, f64, u64)> { Vec::new() }
}
pub use imp::*;
