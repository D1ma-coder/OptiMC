//! Keep FPS deadlines on a fixed clock instead of accumulating OS wakeup error.
use std::time::{Duration, Instant};
pub(super) struct Pacer {
    pub deadline: Instant,
    interval: Option<Duration>,
}
impl Pacer {
    pub fn new(now: Instant) -> Self {
        Self {
            deadline: now,
            interval: None,
        }
    }
    pub fn rendered(&mut self, now: Instant, interval: Option<Duration>) {
        let interval = interval.filter(|period| !period.is_zero());
        if interval != self.interval {
            self.interval = interval;
            self.deadline = now + interval.unwrap_or(Duration::ZERO);
            return;
        }
        let Some(period) = interval else {
            self.deadline = now;
            return;
        };
        if now < self.deadline {
            // Resize/expose can trigger an early redraw. Keep the pacing phase.
            return;
        }
        let late = now.duration_since(self.deadline);
        if late >= period.saturating_mul(4) {
            // Resume after a stall without an overdue catch-up burst.
            self.deadline = now + period;
        } else {
            let steps = (late.as_nanos() / period.as_nanos()) as u32 + 1;
            self.deadline += period * steps;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_late_wakeups_do_not_accumulate_drift() {
        let start = Instant::now();
        let period = Duration::from_nanos(8_333_333);
        let mut clock = Pacer::new(start);
        clock.rendered(start, Some(period));
        for i in 1..=1000 {
            clock.rendered(
                start + period * i + Duration::from_micros(700),
                Some(period),
            );
            assert_eq!(clock.deadline, start + period * (i + 1));
        }
    }
    #[test]
    fn skips_missed_slots_and_retains_phase_on_an_early_expose() {
        let start = Instant::now();
        let period = Duration::from_millis(10);
        let mut clock = Pacer::new(start);
        clock.rendered(start, Some(period));
        clock.rendered(start + Duration::from_millis(3), Some(period));
        assert_eq!(clock.deadline, start + period);
        clock.rendered(start + Duration::from_millis(25), Some(period));
        assert_eq!(clock.deadline, start + Duration::from_millis(30));
        clock.rendered(start + Duration::from_secs(9), Some(period));
        assert_eq!(clock.deadline, start + Duration::from_millis(9010));
    }
    #[test]
    fn changing_cap_or_disabling_it_cannot_keep_an_old_wait() {
        let start = Instant::now();
        let mut clock = Pacer::new(start);
        clock.rendered(start, Some(Duration::from_millis(100)));
        let now = start + Duration::from_millis(10);
        clock.rendered(now, None);
        assert_eq!(clock.deadline, now);
        clock.rendered(now, Some(Duration::from_millis(5)));
        assert_eq!(clock.deadline, now + Duration::from_millis(5));
    }
}
