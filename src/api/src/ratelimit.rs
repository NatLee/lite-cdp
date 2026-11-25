//! 追蹤資料的流量限制：同一個來源在一段時間內只能送幾次。只存在記憶體裡，重開就歸零。

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

pub struct RateLimiter {
    /// 0 是不限制
    limit: u32,
    window: Duration,
    hits: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn new(limit: u32, window: Duration) -> Self {
        Self {
            limit,
            window,
            hits: Mutex::new(HashMap::new()),
        }
    }

    /// 記一次；超過上限時回 `false`。
    pub fn allow(&self, key: &str) -> bool {
        self.allow_at(key, Instant::now())
    }

    fn allow_at(&self, key: &str, now: Instant) -> bool {
        if self.limit == 0 {
            return true;
        }
        let mut hits = self.hits.lock().unwrap();
        // 太多不同的來源時，先把過期的清掉，避免被灌爆
        if hits.len() > 10_000 {
            hits.retain(|_, times| {
                times
                    .last()
                    .is_some_and(|t| now.duration_since(*t) < self.window)
            });
        }
        let times = hits.entry(key.to_owned()).or_default();
        times.retain(|t| now.duration_since(*t) < self.window);
        if times.len() >= self.limit as usize {
            return false;
        }
        times.push(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_per_key_and_recovers_after_the_window() {
        let limiter = RateLimiter::new(2, Duration::from_secs(60));
        let start = Instant::now();
        assert!(limiter.allow_at("a", start));
        assert!(limiter.allow_at("a", start));
        assert!(!limiter.allow_at("a", start));
        assert!(limiter.allow_at("b", start), "別的來源不受影響");
        assert!(limiter.allow_at("a", start + Duration::from_secs(61)));
    }

    #[test]
    fn zero_means_unlimited() {
        let limiter = RateLimiter::new(0, Duration::from_secs(60));
        assert!((0..100).all(|_| limiter.allow("a")));
    }
}
