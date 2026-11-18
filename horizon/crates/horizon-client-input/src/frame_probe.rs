//! iMC (iOS): the engine's frame times, for the app's session log (`platform/ios/App/SessionLog.m`).
//!
//! The game calls [`record`] once for each frame it renders, with how long that frame took. The shell
//! reads and clears the counters every few seconds through the bridge ([`take`]), so the log can say how
//! many frames the engine drew in that window, how long they took on average, the slowest one, and how
//! many ran over one 60 Hz and one 30 Hz frame. Five atomic additions per frame, nothing else: no
//! allocation, no lock, no clock read of its own.
//!
//! Nothing here decides anything or changes what the game does; it only counts. Without the iOS shell
//! nobody calls [`take`] and the counters just keep adding up in place (the totals wrap harmlessly).
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// A frame longer than this missed a 60 Hz refresh (1/60 s, rounded up to 16.7 ms).
pub const SLOW_60_NS: u64 = 16_700_000;
/// A frame longer than this missed a 30 Hz refresh (33.4 ms).
pub const SLOW_30_NS: u64 = 33_400_000;

static FRAMES: AtomicU64 = AtomicU64::new(0);
static SUM_NS: AtomicU64 = AtomicU64::new(0);
static MAX_NS: AtomicU64 = AtomicU64::new(0);
static OVER_60: AtomicU64 = AtomicU64::new(0);
static OVER_30: AtomicU64 = AtomicU64::new(0);

/// What the game recorded since the last [`take`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Window {
    /// Frames rendered.
    pub frames: u64,
    /// The sum of their durations, in nanoseconds (the average is this over `frames`).
    pub sum_ns: u64,
    /// The longest one, in nanoseconds.
    pub max_ns: u64,
    /// How many were longer than [`SLOW_60_NS`].
    pub over_60: u64,
    /// How many were longer than [`SLOW_30_NS`].
    pub over_30: u64,
}

/// One rendered frame that took `ns` nanoseconds.
#[inline]
pub fn record(ns: u64) {
    FRAMES.fetch_add(1, Relaxed);
    SUM_NS.fetch_add(ns, Relaxed);
    MAX_NS.fetch_max(ns, Relaxed);
    if ns > SLOW_60_NS {
        OVER_60.fetch_add(1, Relaxed);
    }
    if ns > SLOW_30_NS {
        OVER_30.fetch_add(1, Relaxed);
    }
}

/// Everything recorded since the last call, and the counters back to zero. A frame recorded while
/// this runs lands in this window or the next, never lost (each counter is swapped on its own, so
/// the five may differ by that one frame).
pub fn take() -> Window {
    Window {
        frames: FRAMES.swap(0, Relaxed),
        sum_ns: SUM_NS.swap(0, Relaxed),
        max_ns: MAX_NS.swap(0, Relaxed),
        over_60: OVER_60.swap(0, Relaxed),
        over_30: OVER_30.swap(0, Relaxed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The counters are process-wide, so the tests that use them take turns.
    static SERIAL: Mutex<()> = Mutex::new(());

    #[test]
    fn a_window_counts_sums_and_finds_the_slowest() {
        let _turn = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        take();
        for ns in [8_000_000, 16_000_000, 20_000_000, 40_000_000] {
            record(ns);
        }
        let w = take();
        assert_eq!(w.frames, 4);
        assert_eq!(w.sum_ns, 84_000_000);
        assert_eq!(w.max_ns, 40_000_000);
        assert_eq!(w.over_60, 2, "20 ms and 40 ms missed 60 Hz");
        assert_eq!(w.over_30, 1, "only 40 ms missed 30 Hz");
    }

    #[test]
    fn taking_clears_the_window() {
        let _turn = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        take();
        record(5_000_000);
        assert_eq!(take().frames, 1);
        assert_eq!(take(), Window::default());
    }

    #[test]
    fn a_frame_exactly_on_the_limit_is_not_slow() {
        let _turn = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        take();
        record(SLOW_60_NS);
        record(SLOW_30_NS);
        let w = take();
        assert_eq!((w.over_60, w.over_30), (1, 0));
    }
}
