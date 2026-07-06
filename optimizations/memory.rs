//! Memory management and optimization

use log::warn;
use rv8_browser_optimizations::memory::{MemoryPressureLevel, MemoryPressureMonitor};
use std::sync::Mutex;
use sysinfo::System;

/// Default warning threshold: 80% of total memory.
const DEFAULT_WARNING_PCT: f64 = 0.8;
/// Default critical threshold: 90% of total memory.
const DEFAULT_CRITICAL_PCT: f64 = 0.9;

/// Memory optimizer for handling pressure and tab discarding.
///
/// Wraps the browser-optimizations `MemoryPressureMonitor` and drives it with
/// live system memory stats from `sysinfo`.
pub struct MemoryOptimizer {
    monitor: MemoryPressureMonitor,
    sys: Mutex<System>,
}

impl Default for MemoryOptimizer {
    fn default() -> Self {
        let mut sys = System::new();
        sys.refresh_memory();
        let total = sys.total_memory() as usize;
        let monitor = MemoryPressureMonitor::new(
            (total as f64 * DEFAULT_WARNING_PCT) as usize,
            (total as f64 * DEFAULT_CRITICAL_PCT) as usize,
        );
        monitor.start_monitoring();
        MemoryOptimizer {
            monitor,
            sys: Mutex::new(sys),
        }
    }
}

impl MemoryOptimizer {
    /// Check if under memory pressure.
    pub fn is_under_pressure(&self) -> bool {
        if let Ok(mut sys) = self.sys.lock() {
            sys.refresh_memory();
            let total = sys.total_memory() as usize;
            let used = sys.used_memory() as usize;
            if total > 0 {
                self.monitor.update_usage(used);
                return self.monitor.get_pressure_level() != MemoryPressureLevel::Normal;
            }
        }
        false
    }

    /// Handle memory pressure.
    pub fn handle_pressure(&self) {
        match self.monitor.get_pressure_level() {
            MemoryPressureLevel::Normal => {}
            MemoryPressureLevel::Warning => {
                warn!("Memory pressure warning: consider evicting background tabs");
            }
            MemoryPressureLevel::Critical => {
                warn!("Memory pressure critical: aggressive tab eviction recommended");
            }
        }
    }

    /// Access the underlying pressure monitor.
    pub fn monitor(&self) -> &MemoryPressureMonitor {
        &self.monitor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_under_pressure_happy_path() {
        let optimizer = MemoryOptimizer::default();
        // Default System reads real system stats — can't assert on actual value.
        // This just ensures no crash and that the monitor is driven.
        let _ = optimizer.is_under_pressure();
    }

    #[test]
    fn test_handle_pressure_does_not_panic() {
        let optimizer = MemoryOptimizer::default();
        optimizer.handle_pressure();
    }
}
