//! Web API implementations for V8
//!
//! Standard browser APIs exposed to JavaScript via V8.

use log::{debug, info};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Console API for JavaScript
pub struct ConsoleApi {
    log_buffer: Vec<ConsoleEntry>,
}

#[derive(Debug, Clone)]
pub struct ConsoleEntry {
    pub level: ConsoleLevel,
    pub message: String,
    pub timestamp: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConsoleLevel {
    Log,
    Info,
    Warn,
    Error,
    Debug,
}

impl ConsoleApi {
    pub fn new() -> Self {
        ConsoleApi {
            log_buffer: Vec::new(),
        }
    }

    pub fn log(&mut self, message: &str) {
        debug!("[JS] console.log: {}", message);
        self.log_buffer.push(ConsoleEntry {
            level: ConsoleLevel::Log,
            message: message.to_string(),
            timestamp: Instant::now(),
        });
    }

    pub fn info(&mut self, message: &str) {
        info!("[JS] console.info: {}", message);
        self.log_buffer.push(ConsoleEntry {
            level: ConsoleLevel::Info,
            message: message.to_string(),
            timestamp: Instant::now(),
        });
    }

    pub fn warn(&mut self, message: &str) {
        debug!("[JS] console.warn: {}", message);
        self.log_buffer.push(ConsoleEntry {
            level: ConsoleLevel::Warn,
            message: message.to_string(),
            timestamp: Instant::now(),
        });
    }

    pub fn error(&mut self, message: &str) {
        debug!("[JS] console.error: {}", message);
        self.log_buffer.push(ConsoleEntry {
            level: ConsoleLevel::Error,
            message: message.to_string(),
            timestamp: Instant::now(),
        });
    }

    pub fn get_logs(&self) -> &[ConsoleEntry] {
        &self.log_buffer
    }

    pub fn clear(&mut self) {
        self.log_buffer.clear();
    }
}

impl Default for ConsoleApi {
    fn default() -> Self {
        Self::new()
    }
}

/// Timer handle for setTimeout/setInterval
pub type TimerId = u64;

/// Timer entry
#[derive(Debug)]
pub struct Timer {
    pub id: TimerId,
    pub callback_id: u64, // ID of callback in V8
    pub fire_time: Instant,
    pub interval: Option<Duration>, // Some for intervals, None for timeouts
    pub cancelled: bool,
}

/// Timer manager for setTimeout/setInterval
pub struct TimerManager {
    timers: HashMap<TimerId, Timer>,
    next_id: TimerId,
}

impl TimerManager {
    pub fn new() -> Self {
        TimerManager {
            timers: HashMap::new(),
            next_id: 1,
        }
    }

    /// Create a setTimeout
    pub fn set_timeout(&mut self, callback_id: u64, delay_ms: u64) -> TimerId {
        let id = self.next_id;
        self.next_id += 1;

        self.timers.insert(
            id,
            Timer {
                id,
                callback_id,
                fire_time: Instant::now() + Duration::from_millis(delay_ms),
                interval: None,
                cancelled: false,
            },
        );

        id
    }

    /// Create a setInterval
    pub fn set_interval(&mut self, callback_id: u64, interval_ms: u64) -> TimerId {
        let id = self.next_id;
        self.next_id += 1;

        let interval = Duration::from_millis(interval_ms);
        self.timers.insert(
            id,
            Timer {
                id,
                callback_id,
                fire_time: Instant::now() + interval,
                interval: Some(interval),
                cancelled: false,
            },
        );

        id
    }

    /// Clear a timer (clearTimeout/clearInterval)
    pub fn clear_timer(&mut self, id: TimerId) {
        self.timers.remove(&id);
    }

    /// Get timers that are ready to fire
    pub fn poll_ready_timers(&mut self) -> Vec<Timer> {
        let now = Instant::now();
        let mut ready = Vec::new();
        let mut to_reschedule = Vec::new();

        for (&id, timer) in &self.timers {
            if !timer.cancelled && timer.fire_time <= now {
                ready.push(Timer {
                    id: timer.id,
                    callback_id: timer.callback_id,
                    fire_time: timer.fire_time,
                    interval: timer.interval,
                    cancelled: false,
                });

                // Reschedule intervals
                if let Some(interval) = timer.interval {
                    to_reschedule.push((id, interval));
                }
            }
        }

        // Remove fired timeouts, reschedule intervals
        for timer in &ready {
            if timer.interval.is_none() {
                self.timers.remove(&timer.id);
            }
        }

        for (id, interval) in to_reschedule {
            if let Some(timer) = self.timers.get_mut(&id) {
                timer.fire_time = now + interval;
            }
        }

        // Clean up cancelled timers
        self.timers.retain(|_, t| !t.cancelled);

        ready
    }
}

impl Default for TimerManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Storage API (localStorage/sessionStorage)
pub struct StorageApi {
    data: HashMap<String, String>,
    max_size: usize,
}

impl StorageApi {
    pub fn new(max_size: usize) -> Self {
        StorageApi {
            data: HashMap::new(),
            max_size,
        }
    }

    pub fn get_item(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(|s| s.as_str())
    }

    pub fn set_item(&mut self, key: &str, value: &str) -> Result<(), String> {
        let total_size: usize = self.data.iter().map(|(k, v)| k.len() + v.len()).sum();

        if total_size + key.len() + value.len() > self.max_size {
            return Err("QuotaExceededError".to_string());
        }

        self.data.insert(key.to_string(), value.to_string());
        Ok(())
    }

    pub fn remove_item(&mut self, key: &str) {
        self.data.remove(key);
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn key(&self, index: usize) -> Option<&str> {
        self.data.keys().nth(index).map(|s| s.as_str())
    }

    pub fn length(&self) -> usize {
        self.data.len()
    }
}

impl Default for StorageApi {
    fn default() -> Self {
        Self::new(5 * 1024 * 1024) // 5MB default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_console_api() {
        let mut console = ConsoleApi::new();

        console.log("Test log");
        console.info("Test info");
        console.warn("Test warn");
        console.error("Test error");

        let logs = console.get_logs();
        assert_eq!(logs.len(), 4);

        assert_eq!(logs[0].level, ConsoleLevel::Log);
        assert_eq!(logs[0].message, "Test log");

        assert_eq!(logs[1].level, ConsoleLevel::Info);
        assert_eq!(logs[1].message, "Test info");

        assert_eq!(logs[2].level, ConsoleLevel::Warn);
        assert_eq!(logs[2].message, "Test warn");

        assert_eq!(logs[3].level, ConsoleLevel::Error);
        assert_eq!(logs[3].message, "Test error");

        console.clear();
        assert_eq!(console.get_logs().len(), 0);
    }

    #[test]
    fn test_timer_manager_timeout() {
        let mut timers = TimerManager::new();

        let cb_id = 42;
        let delay_ms = 10;
        let timer_id = timers.set_timeout(cb_id, delay_ms);

        // Initially, the timer should not be ready
        assert_eq!(timers.poll_ready_timers().len(), 0);

        // Wait for the timeout
        std::thread::sleep(Duration::from_millis(15));

        let ready = timers.poll_ready_timers();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, timer_id);
        assert_eq!(ready[0].callback_id, cb_id);
        assert!(ready[0].interval.is_none());

        // Timer should be removed after firing
        assert_eq!(timers.poll_ready_timers().len(), 0);
    }

    #[test]
    fn test_timer_manager_interval() {
        let mut timers = TimerManager::new();

        let cb_id = 43;
        let interval_ms = 10;
        let timer_id = timers.set_interval(cb_id, interval_ms);

        // Wait for the first interval
        std::thread::sleep(Duration::from_millis(15));

        let ready = timers.poll_ready_timers();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, timer_id);
        assert_eq!(ready[0].interval, Some(Duration::from_millis(interval_ms)));

        // Wait for the second interval
        std::thread::sleep(Duration::from_millis(15));

        let ready2 = timers.poll_ready_timers();
        assert_eq!(ready2.len(), 1);
        assert_eq!(ready2[0].id, timer_id);
    }

    #[test]
    fn test_timer_manager_clear() {
        let mut timers = TimerManager::new();

        let timer_id = timers.set_timeout(44, 10);
        assert_eq!(timers.timers.len(), 1);

        timers.clear_timer(timer_id);
        assert!(timers.timers.is_empty());

        std::thread::sleep(Duration::from_millis(15));
        assert_eq!(timers.poll_ready_timers().len(), 0);
    }

    #[test]
    fn test_storage_api_basic() {
        let mut storage = StorageApi::new(1024);

        assert_eq!(storage.length(), 0);
        assert_eq!(storage.get_item("nonexistent"), None);

        assert!(storage.set_item("key1", "value1").is_ok());
        assert_eq!(storage.get_item("key1"), Some("value1"));
        assert_eq!(storage.length(), 1);
        assert_eq!(storage.key(0), Some("key1"));

        assert!(storage.set_item("key2", "value2").is_ok());
        assert_eq!(storage.length(), 2);

        storage.remove_item("key1");
        assert_eq!(storage.get_item("key1"), None);
        assert_eq!(storage.length(), 1);

        storage.clear();
        assert_eq!(storage.length(), 0);
        assert_eq!(storage.key(0), None);
    }

    #[test]
    fn test_storage_api_quota() {
        // Create storage with only 10 bytes quota
        let mut storage = StorageApi::new(10);

        // This is 4 (key) + 5 (value) = 9 bytes. Should succeed.
        assert!(storage.set_item("key1", "value").is_ok());

        // Adding another item would exceed 10 bytes total size
        let result = storage.set_item("k2", "v2");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "QuotaExceededError");

        // Ensure the second item was not added
        assert_eq!(storage.get_item("k2"), None);
        assert_eq!(storage.length(), 1);
    }
}
