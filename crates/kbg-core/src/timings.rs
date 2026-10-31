/// Durations counted in power-of-two buckets of nanoseconds: fixed size and no allocation, so
/// it can time a hook procedure on every call.
#[derive(Clone, Debug, Default)]
pub struct Timings {
    /// Bucket `b` holds durations of `2^(b-1)` up to `2^b` ns (bucket 0: 0 ns).
    buckets: [u32; 32],
    count: u64,
    max_ns: u64,
}

impl Timings {
    pub fn record(&mut self, ns: u64) {
        let bucket = (u64::BITS - ns.leading_zeros()) as usize;
        let slot = &mut self.buckets[bucket.min(31)];
        *slot = slot.saturating_add(1);
        self.count += 1;
        self.max_ns = self.max_ns.max(ns);
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    pub fn max_ns(&self) -> u64 {
        self.max_ns
    }

    /// An upper bound for the `q` quantile (0.0..=1.0) in ns: the top of its bucket, at most
    /// the longest duration seen.
    pub fn quantile_ns(&self, q: f64) -> u64 {
        let target = ((self.count as f64 * q).ceil() as u64).max(1);
        let mut seen = 0;
        for (bucket, &n) in self.buckets.iter().enumerate() {
            seen += u64::from(n);
            if seen >= target {
                return match bucket {
                    0 => 0,
                    // The last bucket has no top.
                    31 => self.max_ns,
                    _ => (1u64 << bucket).min(self.max_ns),
                };
            }
        }
        self.max_ns
    }

    /// `1234 calls, p50 <= 2.0 µs, p99 <= 8.2 µs, max 15.3 µs` for the log.
    pub fn summary(&self) -> String {
        let us = |ns: u64| ns as f64 / 1000.0;
        format!(
            "{} calls, p50 <= {:.1} µs, p99 <= {:.1} µs, max {:.1} µs",
            self.count,
            us(self.quantile_ns(0.5)),
            us(self.quantile_ns(0.99)),
            us(self.max_ns)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantiles_are_bucket_tops() {
        let mut t = Timings::default();
        for _ in 0..99 {
            t.record(1_500); // bucket 1024..2048
        }
        t.record(40_000); // bucket 32768..65536
        assert_eq!(t.count(), 100);
        assert_eq!(t.quantile_ns(0.5), 2_048);
        assert_eq!(t.quantile_ns(0.99), 2_048);
        // Never above the longest one.
        assert_eq!(t.quantile_ns(1.0), 40_000);
        assert_eq!(t.max_ns(), 40_000);
    }

    #[test]
    fn empty_and_huge() {
        let mut t = Timings::default();
        assert_eq!(t.quantile_ns(0.99), 0);
        t.record(0);
        assert_eq!(t.quantile_ns(0.5), 0);
        t.record(u64::MAX);
        assert_eq!(t.quantile_ns(1.0), u64::MAX);
    }
}
