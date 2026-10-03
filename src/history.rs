use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub at: Instant,
    pub values: Option<[f64; 2]>,
}
pub struct History {
    pub points: VecDeque<Point>,
    capacity: usize,
    pub identity: String,
}
impl History {
    pub fn new(capacity: usize) -> Self {
        Self {
            points: VecDeque::with_capacity(capacity),
            capacity,
            identity: String::new(),
        }
    }
    pub fn push(&mut self, identity: &str, at: Instant, values: Option<[f64; 2]>) {
        if self.identity != identity {
            self.points.clear();
            self.identity = identity.to_string();
        }
        if self.points.back().is_some_and(|p| p.at >= at) {
            return;
        }
        if self.points.len() == self.capacity {
            self.points.pop_front();
        }
        self.points.push_back(Point { at, values });
    }
    /// Preserve gaps and timestamp spacing; don't join errors, resets, pauses or skipped frames.
    pub fn segments(
        &self,
        now: Instant,
        window: Duration,
        interval: Duration,
        series: usize,
        scale: f64,
    ) -> Vec<Vec<(f64, f64)>> {
        let mut segments: Vec<Vec<(f64, f64)>> = Vec::new();
        let mut current = Vec::new();
        let mut previous = None;
        for point in &self.points {
            let age = now.saturating_duration_since(point.at);
            if age > window {
                continue;
            }
            let gap = previous.is_some_and(|t| point.at.duration_since(t) > interval.mul_f64(1.8));
            if (gap || point.values.is_none()) && !current.is_empty() {
                segments.push(std::mem::take(&mut current));
            }
            if let Some(values) = point.values {
                current.push((-age.as_secs_f64(), values[series] / scale));
            }
            previous = Some(point.at);
        }
        if !current.is_empty() {
            segments.push(current);
        }
        segments
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_timestamp_spacing_and_gaps() {
        let t = Instant::now();
        let mut h = History::new(3);
        h.push("eth0", t, Some([1., 2.]));
        h.push("eth0", t + Duration::from_secs(1), None);
        h.push("eth0", t + Duration::from_millis(2500), Some([3., 4.]));
        let parts = h.segments(
            t + Duration::from_secs(3),
            Duration::from_secs(10),
            Duration::from_secs(1),
            0,
            1.,
        );
        assert_eq!(parts, vec![vec![(-3., 1.)], vec![(-0.5, 3.)]]);
        h.push("eth0", t + Duration::from_secs(3), Some([4., 5.]));
        assert_eq!(h.points.len(), 3);
        h.push("wlan0", t + Duration::from_secs(4), Some([5., 6.]));
        assert_eq!(h.points.len(), 1);
        h.push("wlan0", t + Duration::from_secs(4), Some([6., 7.]));
        assert_eq!(h.points.len(), 1);
    }
}
