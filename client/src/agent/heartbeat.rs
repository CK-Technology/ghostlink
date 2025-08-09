use anyhow::Result;
use tokio::time::{interval, Duration, Instant};
use tracing::{debug, error, info, warn};

/// Manages heartbeat communication with the server
pub struct HeartbeatManager {
    interval_duration: Duration,
    last_heartbeat: Option<Instant>,
    consecutive_failures: u32,
    max_failures: u32,
}

impl HeartbeatManager {
    pub fn new(interval_seconds: u64) -> Self {
        Self {
            interval_duration: Duration::from_secs(interval_seconds),
            last_heartbeat: None,
            consecutive_failures: 0,
            max_failures: 3,
        }
    }

    /// Record successful heartbeat
    pub fn record_success(&mut self) {
        self.last_heartbeat = Some(Instant::now());
        
        if self.consecutive_failures > 0 {
            info!("Heartbeat recovered after {} failures", self.consecutive_failures);
            self.consecutive_failures = 0;
        }
        
        debug!("Heartbeat successful");
    }

    /// Record failed heartbeat
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        warn!("Heartbeat failed (attempt {})", self.consecutive_failures);
    }

    /// Check if connection should be considered dead
    pub fn is_connection_dead(&self) -> bool {
        self.consecutive_failures >= self.max_failures
    }

    /// Get time since last successful heartbeat
    pub fn time_since_last_heartbeat(&self) -> Option<Duration> {
        self.last_heartbeat.map(|instant| instant.elapsed())
    }

    /// Check if heartbeat is overdue
    pub fn is_overdue(&self) -> bool {
        if let Some(last) = self.last_heartbeat {
            last.elapsed() > self.interval_duration * 2
        } else {
            true // No heartbeat yet
        }
    }

    /// Get health status
    pub fn get_health_status(&self) -> HeartbeatHealth {
        if self.is_connection_dead() {
            HeartbeatHealth::Dead
        } else if self.consecutive_failures > 0 {
            HeartbeatHealth::Degraded
        } else if self.is_overdue() {
            HeartbeatHealth::Overdue
        } else {
            HeartbeatHealth::Healthy
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum HeartbeatHealth {
    Healthy,
    Overdue,
    Degraded,
    Dead,
}

/// Heartbeat message sent to server
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HeartbeatMessage {
    pub timestamp: u64,
    pub agent_id: String,
    pub uptime: u64,
    pub active_sessions: u32,
    pub system_load: Option<f32>,
    pub memory_usage: Option<u64>,
}

impl HeartbeatMessage {
    pub fn new(agent_id: String, active_sessions: u32) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let uptime = Self::get_system_uptime();
        let (system_load, memory_usage) = Self::get_system_metrics();

        Self {
            timestamp,
            agent_id,
            uptime,
            active_sessions,
            system_load,
            memory_usage,
        }
    }

    fn get_system_uptime() -> u64 {
        use sysinfo::System;
        System::uptime()
    }

    fn get_system_metrics() -> (Option<f32>, Option<u64>) {
        use sysinfo::System;
        
        let mut sys = System::new();
        sys.refresh_memory();
        
        // Memory usage as percentage
        let memory_usage = if sys.total_memory() > 0 {
            Some(((sys.total_memory() - sys.available_memory()) * 100) / sys.total_memory())
        } else {
            None
        };

        // System load (simplified - would need platform-specific implementation)
        let system_load = None; // TODO: Implement per-platform

        (system_load, memory_usage)
    }
}
