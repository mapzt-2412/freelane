//! Fair dispatch policy for the single-worker snapshot viewer.
use std::time::{Duration, Instant};
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Idle,
    Input,
    Capture,
}
pub struct Schedule {
    next: Instant,
    completed_inputs: u8,
}
impl Schedule {
    pub fn new(now: Instant) -> Self {
        Self {
            next: now,
            completed_inputs: 0,
        }
    }
    pub fn choose(
        &self,
        now: Instant,
        busy: bool,
        live: bool,
        has_target: bool,
        queued: bool,
    ) -> Decision {
        if busy {
            Decision::Idle
        } else if live && has_target && now >= self.next && (self.completed_inputs >= 8 || !queued)
        {
            Decision::Capture
        } else if queued {
            Decision::Input
        } else {
            Decision::Idle
        }
    }
    pub fn input_completed(&mut self, now: Instant, queue_empty: bool) {
        self.completed_inputs = self.completed_inputs.saturating_add(1).min(8);
        if queue_empty {
            self.next = self.next.min(now);
        }
    }
    pub fn frame_completed(&mut self, now: Instant) {
        self.completed_inputs = 0;
        self.next = now + Duration::from_secs(1);
    }
    pub fn reset(&mut self, now: Instant) {
        *self = Self::new(now);
    }
    pub fn deadline(&self) -> Instant {
        self.next
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn due_frame_gets_a_turn_after_eight_inputs() {
        let now = Instant::now();
        let mut s = Schedule::new(now);
        for _ in 0..8 {
            assert_eq!(s.choose(now, false, true, true, true), Decision::Input);
            s.input_completed(now, false);
        }
        assert_eq!(s.choose(now, false, true, true, true), Decision::Capture);
    }
    #[test]
    fn busy_never_dispatches() {
        let now = Instant::now();
        let s = Schedule::new(now);
        assert_eq!(s.choose(now, true, true, true, false), Decision::Idle);
        assert_eq!(s.choose(now, true, true, true, true), Decision::Idle);
    }
    #[test]
    fn live_disabled_keeps_input() {
        let now = Instant::now();
        let mut s = Schedule::new(now);
        for _ in 0..40 {
            s.input_completed(now, false);
        }
        assert_eq!(s.choose(now, false, false, true, true), Decision::Input);
        assert_eq!(s.choose(now, false, false, true, false), Decision::Idle);
    }
    #[test]
    fn future_frame_does_not_delay_input() {
        let now = Instant::now();
        let mut s = Schedule::new(now);
        s.frame_completed(now);
        for _ in 0..20 {
            s.input_completed(now, false);
        }
        assert_eq!(s.choose(now, false, true, true, true), Decision::Input);
        assert_eq!(
            s.choose(now + Duration::from_secs(1), false, true, true, true),
            Decision::Capture
        );
    }
    #[test]
    fn idle_due_frame_dispatches() {
        let now = Instant::now();
        let s = Schedule::new(now);
        assert_eq!(s.choose(now, false, true, true, false), Decision::Capture);
        assert_eq!(s.choose(now, false, true, false, false), Decision::Idle);
    }
    #[test]
    fn input_does_not_postpone_due_frame() {
        let now = Instant::now();
        let mut s = Schedule::new(now);
        s.input_completed(now + Duration::from_secs(2), true);
        assert_eq!(s.deadline(), now);
        s.frame_completed(now);
        s.input_completed(now + Duration::from_millis(100), true);
        assert_eq!(s.deadline(), now + Duration::from_millis(100));
    }
    #[test]
    fn reset_clears_burst() {
        let now = Instant::now();
        let mut s = Schedule::new(now);
        for _ in 0..8 {
            s.input_completed(now, false);
        }
        s.reset(now);
        assert_eq!(s.choose(now, false, true, true, true), Decision::Input);
        assert_eq!(s.deadline(), now);
    }
    #[test]
    fn frame_completion_sets_one_second_deadline() {
        let now = Instant::now();
        let mut s = Schedule::new(now);
        for _ in 0..8 {
            s.input_completed(now, false);
        }
        s.frame_completed(now);
        assert_eq!(s.deadline(), now + Duration::from_secs(1));
        assert_eq!(
            s.choose(now + Duration::from_secs(1), false, true, true, true),
            Decision::Input
        );
    }
}
