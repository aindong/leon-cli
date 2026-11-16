//! The working indicators' one clock (ADR R6): a `cx.spawn` loop on a 125 ms timer that notifies,
//! running only while some shown indicator works (the frame drawn is `Workspace::pulse`, from the
//! clock). The views that show one observe it, so every spinner moves on the same tick and a
//! tick re-renders only them.

use gpui::{Context, Task};

use super::SPIN;

pub struct Pulse {
    ticks: u64,
    /// Ticks since a frame was drawn: a window that draws none (hidden, covered) shows no
    /// spinner, so from the second the clock waits for the next frame (`drew`), whatever else
    /// asks it to run meanwhile.
    undrawn: u8,
    paused: bool,
    task: Option<Task<()>>,
}

impl Pulse {
    pub fn new() -> Self {
        Pulse { ticks: 0, undrawn: 0, paused: false, task: None }
    }

    /// How many times it ticked (tests).
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Whether the clock runs.
    pub fn running(&self) -> bool {
        self.task.is_some()
    }

    /// Run while something shown works; stop when nothing does.
    pub fn set_running(&mut self, on: bool, cx: &mut Context<Self>) {
        match (on, self.task.is_some()) {
            (true, false) if !self.paused => self.start(cx),
            (false, true) => self.task = None,
            _ => {}
        }
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        self.undrawn = 0;
        self.task = Some(cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(SPIN).await;
            let more = this.update(cx, |p, cx| {
                p.undrawn += 1;
                if p.undrawn > 2 {
                    p.paused = true;
                    p.task = None; // dropping the task ends this loop at its next await
                    return false;
                }
                p.ticks += 1;
                cx.notify();
                true
            });
            if !more.unwrap_or(false) {
                return;
            }
        }));
    }

    /// A frame was drawn: a clock paused for want of frames starts again while still wanted.
    pub fn drew(&mut self, wanted: bool, cx: &mut Context<Self>) {
        (self.undrawn, self.paused) = (0, false);
        if wanted && self.task.is_none() {
            self.start(cx);
        }
    }
}

impl Default for Pulse {
    fn default() -> Self {
        Self::new()
    }
}
