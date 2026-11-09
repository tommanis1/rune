//! Why a window draws, for a window that draws when nothing seems to change: with `RUNE_FRAME_WHY=FILE`, one JSON
//! line per second that had frames, appended to FILE. Each line counts the frames, the commits by
//! [`CommitWhy`](super::CommitWhy) bit, what kept the frames animating by [`AnimSource`](super::AnimSource) bit (and
//! the frame passes that animate without a source bit: `build`, `rects`, `rows`, `reports`), the keys of the
//! elements that loop, the app handlers that ran (and so committed), and the frames' total and slowest times. Unlike the headless frame log (`RUNE_FRAME_LOG`), it
//! works in a real window and costs nothing when unset.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write;
use std::time::Instant;

use super::{AnimSource, CommitWhy};

/// The frame passes that can keep a frame animating without a source bit.
pub(crate) const OTHER: [&str; 4] = ["build", "rects", "rows", "reports"];

struct Log {
    out: std::fs::File,
    second: Instant,
    frames: u32,
    commits: u32,
    why: [u32; 10],
    anim: [u32; 10],
    other: [u32; 4],
    loops: BTreeMap<String, u32>,
    handlers: BTreeMap<String, u32>,
    total_ms: f64,
    max_ms: f64,
}

thread_local! {
    static LOG: RefCell<Option<Log>> = RefCell::new(
        std::env::var_os("RUNE_FRAME_WHY")
            .and_then(|p| std::fs::OpenOptions::new().create(true).append(true).open(p).ok())
            .map(|out| Log { out, second: Instant::now(), frames: 0, commits: 0, why: [0; 10], anim: [0; 10], other: [0; 4], loops: BTreeMap::new(), handlers: BTreeMap::new(), total_ms: 0.0, max_ms: 0.0 }),
    );
}

/// Whether the log is on (so callers skip gathering what it would not write).
pub(crate) fn on() -> bool {
    LOG.with(|l| l.borrow().is_some())
}

/// A frame pass (`OTHER[i]`) kept this frame animating.
pub(crate) fn other(i: usize, animating: bool) {
    if animating {
        LOG.with(|l| if let Some(l) = l.borrow_mut().as_mut() { l.other[i] += 1 });
    }
}

/// An app handler ran (`on_press@path`), so the frame commits for input.
pub(crate) fn handler(h: String) {
    LOG.with(|l| if let Some(l) = l.borrow_mut().as_mut() { *l.handlers.entry(h).or_default() += 1 });
}

/// One frame ended: it took MS, committed for WHY (0: no commit), animated by ANIM bits when ANIMATING, with the
/// looping elements LOOPS.
pub(crate) fn frame(ms: f64, why: u16, animating: bool, anim: u16, loops: impl Iterator<Item = String>) {
    LOG.with(|l| {
        let mut l = l.borrow_mut();
        let Some(l) = l.as_mut() else { return };
        l.frames += 1;
        l.total_ms += ms;
        l.max_ms = l.max_ms.max(ms);
        if why != 0 {
            l.commits += 1;
            (0..10).filter(|i| why & (1 << i) != 0).for_each(|i| l.why[i] += 1);
        }
        if animating {
            (0..10).filter(|i| anim & (1 << i) != 0).for_each(|i| l.anim[i] += 1);
            if anim & AnimSource::LOOP != 0 {
                for k in loops {
                    *l.loops.entry(k).or_default() += 1;
                }
            }
        }
        if l.second.elapsed().as_secs_f64() >= 1.0 {
            let named = |names: &[&str], n: &[u32]| -> serde_json::Map<String, serde_json::Value> {
                names.iter().zip(n).filter(|(_, n)| **n > 0).map(|(k, n)| (k.to_string(), (*n).into())).collect()
            };
            let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
            let line = serde_json::json!({
                "at": (at * 10.0).round() / 10.0,
                "frames": l.frames,
                "commits": l.commits,
                "why": named(&CommitWhy::NAMES, &l.why),
                "anim": named(&AnimSource::NAMES, &l.anim),
                "other": named(&OTHER, &l.other),
                "loops": l.loops.iter().take(8).map(|(k, n)| (k.clone(), serde_json::Value::from(*n))).collect::<serde_json::Map<_, _>>(),
                "handlers": l.handlers.iter().take(8).map(|(k, n)| (k.clone(), serde_json::Value::from(*n))).collect::<serde_json::Map<_, _>>(),
                "ms": (l.total_ms * 10.0).round() / 10.0,
                "max_ms": (l.max_ms * 10.0).round() / 10.0,
            });
            let _ = writeln!(l.out, "{line}");
            l.second = Instant::now();
            (l.frames, l.commits, l.total_ms, l.max_ms) = (0, 0, 0.0, 0.0);
            (l.why, l.anim, l.other) = ([0; 10], [0; 10], [0; 4]);
            l.loops.clear();
            l.handlers.clear();
        }
    });
}
