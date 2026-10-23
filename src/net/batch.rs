//! Original 26.2 ChunkBatchSizeCalculator smoothing and adaptive flow control.
use std::time::Instant;
pub struct Calculator {
    start: Instant,
    nanos: f64,
    weight: u32,
}
impl Default for Calculator {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            nanos: 2_000_000.,
            weight: 1,
        }
    }
}
impl Calculator {
    pub fn start(&mut self) {
        self.start = Instant::now();
    }
    pub fn finish(&mut self, count: i32) -> f32 {
        self.sample(count, self.start.elapsed().as_nanos() as f64);
        self.desired()
    }
    fn sample(&mut self, count: i32, elapsed: f64) {
        if count <= 0 {
            return;
        }
        let sample = (elapsed / count as f64).clamp(self.nanos / 3., self.nanos * 3.);
        self.nanos = (self.nanos * self.weight as f64 + sample) / (self.weight + 1) as f64;
        self.weight = (self.weight + 1).min(49);
    }
    fn desired(&self) -> f32 {
        (7_000_000. / self.nanos) as f32
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_smoothing_and_empty_batches() {
        let mut c = Calculator::default();
        assert_eq!(c.desired(), 3.5);
        c.sample(0, 1.);
        assert_eq!(c.desired(), 3.5);
        c.sample(1, 100_000_000.);
        assert_eq!(c.desired(), 1.75);
        c.sample(1, 1.);
        assert!((c.desired() - 2.25).abs() < 1e-6);
        for _ in 0..100 {
            c.sample(2, 4_000_000.);
        }
        assert_eq!(c.weight, 49);
        assert!(c.desired().is_finite());
    }
}
