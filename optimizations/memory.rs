//! Memory management and optimization

use log::warn;
use std::sync::Mutex;
use sysinfo::System;

/// Trait for system monitoring to allow mocking in tests
pub trait SystemMonitor: Send {
    /// Refresh memory statistics
    fn refresh_memory(&mut self);
    /// Get total memory
    fn total_memory(&self) -> u64;
    /// Get used memory
    fn used_memory(&self) -> u64;
}

impl SystemMonitor for System {
    fn refresh_memory(&mut self) {
        System::refresh_memory(self);
    }

    fn total_memory(&self) -> u64 {
        System::total_memory(self)
    }

    fn used_memory(&self) -> u64 {
        System::used_memory(self)
    }
}

/// Memory optimizer for handling pressure and tab discarding
pub struct MemoryOptimizer {
    /// Memory pressure threshold (percentage)
    pressure_threshold: f64,
    /// Enable tab discarding
    tab_discarding: bool,
    /// Enable tab freezing
    tab_freezing: bool,
    /// System monitoring
    sys: Mutex<Box<dyn SystemMonitor>>,
}

impl MemoryOptimizer {
    /// Create a new memory optimizer with a specific monitor
    pub fn new_with_monitor(monitor: Box<dyn SystemMonitor>) -> Self {
        MemoryOptimizer {
            pressure_threshold: 0.8,
            tab_discarding: true,
            tab_freezing: true,
            sys: Mutex::new(monitor),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSystem {
        total: u64,
        used: u64,
    }

    impl SystemMonitor for MockSystem {
        fn refresh_memory(&mut self) {}

        fn total_memory(&self) -> u64 {
            self.total
        }

        fn used_memory(&self) -> u64 {
            self.used
        }
    }

    #[test]
    fn test_is_under_pressure_happy_path() {
        let optimizer = MemoryOptimizer::new_with_monitor(Box::new(MockSystem {
            total: 1000,
            used: 500, // 50% usage
        }));
        assert!(!optimizer.is_under_pressure());
    }

    #[test]
    fn test_is_under_pressure_under_pressure() {
        let optimizer = MemoryOptimizer::new_with_monitor(Box::new(MockSystem {
            total: 1000,
            used: 900, // 90% usage
        }));
        assert!(optimizer.is_under_pressure());
    }

    #[test]
    fn test_is_under_pressure_zero_total_memory() {
        let optimizer =
            MemoryOptimizer::new_with_monitor(Box::new(MockSystem { total: 0, used: 0 }));
        assert!(!optimizer.is_under_pressure());
    }

    #[test]
    fn test_is_under_pressure_exact_threshold() {
        let optimizer = MemoryOptimizer::new_with_monitor(Box::new(MockSystem {
            total: 1000,
            used: 800, // exactly 80% usage (default threshold)
        }));
        assert!(optimizer.is_under_pressure());
    }
}

impl Default for MemoryOptimizer {
    fn default() -> Self {
        MemoryOptimizer::new_with_monitor(Box::new(System::new()))
    }
}

impl MemoryOptimizer {
    /// Check if under memory pressure
    pub fn is_under_pressure(&self) -> bool {
        if let Ok(mut sys) = self.sys.lock() {
            sys.refresh_memory();
            let total = sys.total_memory();
            let used = sys.used_memory();
            if total > 0 {
                let usage = used as f64 / total as f64;
                return usage >= self.pressure_threshold;
            }
        }
        false
    }

    /// Handle memory pressure
    pub fn handle_pressure(&self) {
        if self.is_under_pressure() {
            warn!("Memory pressure detected");
            // Trigger GC in all renderers
            // Discard background tabs
            // Free cached resources
        }
    }
}
