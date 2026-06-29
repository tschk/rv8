//! Memory management and optimization

use log::warn;
use std::sync::Mutex;
use sysinfo::System;

/// Memory optimizer for handling pressure and tab discarding
pub struct MemoryOptimizer {
    /// Memory pressure threshold (percentage)
    pressure_threshold: f64,
    /// System monitoring
    sys: Mutex<System>,
}

impl Default for MemoryOptimizer {
    fn default() -> Self {
        MemoryOptimizer {
            pressure_threshold: 0.8,
            sys: Mutex::new(System::new()),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_under_pressure_happy_path() {
        let optimizer = MemoryOptimizer::default();
        // Default System reads real system stats — can't assert on actual value
        // This just ensures no crash
        let _ = optimizer.is_under_pressure();
    }
}
