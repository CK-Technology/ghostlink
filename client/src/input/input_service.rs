use crate::{
    error::{GhostLinkError, Result},
    input::{
        input_protocol::{InputEvent, InputStats, KeyType, MouseButtonType, ScrollDirectionType},
        x11_input::X11InputInjector,
    },
    connection::RelayConnection,
};

use std::{
    sync::{Arc, atomic::{AtomicBool, Ordering}},
    time::{Duration, Instant},
};
use tokio::{
    sync::{RwLock, mpsc},
    task::JoinHandle,
    time::sleep,
};
use tracing::{debug, error, info, trace, warn};
use parking_lot::RwLock as ParkingRwLock;

const INPUT_PROCESSING_INTERVAL_MS: u64 = 1; // Process input every 1ms for low latency
const MAX_EVENTS_PER_BATCH: usize = 50;      // Process up to 50 events per batch
const INPUT_STATS_INTERVAL_SECONDS: u64 = 30; // Log stats every 30 seconds

/// High-performance input service for processing remote control events
pub struct InputService {
    /// Session identifier
    session_id: [u8; 8],
    /// X11 input injector for native input
    #[cfg(target_os = "linux")]
    injector: Arc<RwLock<X11InputInjector>>,
    /// Input event receiver channel
    event_receiver: Arc<RwLock<Option<mpsc::Receiver<InputEvent>>>>,
    /// Input event sender (for external use)
    event_sender: mpsc::Sender<InputEvent>,
    /// Service running state
    is_running: AtomicBool,
    /// Input processing task handle
    processing_task: Arc<RwLock<Option<JoinHandle<()>>>>,
    /// Statistics tracking
    stats: Arc<ParkingRwLock<InputStats>>,
    /// Connection for responses/feedback
    connection: Arc<RelayConnection>,
    /// Input processing configuration
    config: InputServiceConfig,
}

/// Configuration for input service behavior
#[derive(Debug, Clone)]
pub struct InputServiceConfig {
    /// Enable mouse input processing
    pub enable_mouse: bool,
    /// Enable keyboard input processing  
    pub enable_keyboard: bool,
    /// Maximum events per second to prevent flooding
    pub max_events_per_second: u32,
    /// Enable input validation
    pub validate_events: bool,
    /// Log input events for debugging
    pub log_events: bool,
    /// Clipboard access enabled
    pub enable_clipboard: bool,
}

impl Default for InputServiceConfig {
    fn default() -> Self {
        Self {
            enable_mouse: true,
            enable_keyboard: true,
            max_events_per_second: 1000, // Allow up to 1000 events/sec
            validate_events: true,
            log_events: false, // Disabled by default for performance
            enable_clipboard: true,
        }
    }
}

impl InputService {
    /// Create new input service
    pub async fn new(
        session_id: [u8; 8],
        connection: Arc<RelayConnection>,
        config: InputServiceConfig,
    ) -> Result<Self> {
        info!("Creating input service for session {:?}", session_id);
        
        // Create input injector for the current platform
        #[cfg(target_os = "linux")]
        let injector = {
            let injector = X11InputInjector::new()?;
            
            // Test input injection capabilities
            if !injector.test_input_injection()? {
                warn!("Input injection test failed - some features may not work");
            }
            
            Arc::new(RwLock::new(injector))
        };
        
        #[cfg(not(target_os = "linux"))]
        let injector = {
            return Err(GhostLinkError::Other("Input service only supports Linux/X11 currently".to_string()));
        };
        
        // Create event channel
        let (event_sender, event_receiver) = mpsc::channel(1000);
        
        Ok(Self {
            session_id,
            injector,
            event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
            event_sender,
            is_running: AtomicBool::new(false),
            processing_task: Arc::new(RwLock::new(None)),
            stats: Arc::new(ParkingRwLock::new(InputStats::default())),
            connection,
            config,
        })
    }
    
    /// Start the input service
    pub async fn start(&self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            warn!("Input service already running");
            return Ok(());
        }
        
        info!("Starting input service for session {:?}", self.session_id);
        self.is_running.store(true, Ordering::Relaxed);
        
        // Take the receiver from the option
        let mut receiver_guard = self.event_receiver.write().await;
        let receiver = receiver_guard.take()
            .ok_or_else(|| GhostLinkError::Other("Input service already started".to_string()))?;
        drop(receiver_guard);
        
        // Start input processing task
        let task = self.spawn_processing_task(receiver).await;
        *self.processing_task.write().await = Some(task);
        
        info!("Input service started successfully");
        Ok(())
    }
    
    /// Stop the input service
    pub async fn stop(&self) -> Result<()> {
        if !self.is_running.load(Ordering::Relaxed) {
            return Ok(());
        }
        
        info!("Stopping input service for session {:?}", self.session_id);
        self.is_running.store(false, Ordering::Relaxed);
        
        // Stop processing task
        if let Some(task) = self.processing_task.write().await.take() {
            task.abort();
            let _ = task.await;
        }
        
        info!("Input service stopped");
        self.log_final_stats().await;
        Ok(())
    }
    
    /// Send input event to be processed
    pub async fn send_input_event(&self, event: InputEvent) -> Result<()> {
        if !self.is_running.load(Ordering::Relaxed) {
            return Err(GhostLinkError::Other("Input service not running".to_string()));
        }
        
        // Validate event if enabled
        if self.config.validate_events {
            event.validate()?;
        }
        
        // Check rate limiting
        if !self.check_rate_limit().await {
            warn!("Input event rate limit exceeded, dropping event");
            return Err(GhostLinkError::Other("Rate limit exceeded".to_string()));
        }
        
        // Send event to processing queue
        self.event_sender.send(event).await
            .map_err(|e| GhostLinkError::Other(format!("Failed to queue input event: {}", e)))?;
        
        Ok(())
    }
    
    /// Process input event from JSON (for WebSocket messages)
    pub async fn process_input_event_json(&self, json: &str) -> Result<()> {
        let event = InputEvent::from_json(json)?;
        self.send_input_event(event).await
    }
    
    /// Spawn the main input processing task
    async fn spawn_processing_task(&self, mut receiver: mpsc::Receiver<InputEvent>) -> JoinHandle<()> {
        let injector = Arc::clone(&self.injector);
        let stats = Arc::clone(&self.stats);
        let is_running = Arc::clone(&self.is_running);
        let config = self.config.clone();
        let session_id = self.session_id;
        
        tokio::spawn(async move {
            info!("Input processing task started for session {:?}", session_id);
            
            let mut interval = tokio::time::interval(Duration::from_millis(INPUT_PROCESSING_INTERVAL_MS));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            let mut stats_timer = Instant::now();
            let mut event_batch = Vec::with_capacity(MAX_EVENTS_PER_BATCH);
            
            while is_running.load(Ordering::Relaxed) {
                interval.tick().await;
                
                // Collect events from the channel (non-blocking)
                event_batch.clear();
                while event_batch.len() < MAX_EVENTS_PER_BATCH {
                    match receiver.try_recv() {
                        Ok(event) => event_batch.push(event),
                        Err(mpsc::error::TryRecvError::Empty) => break,
                        Err(mpsc::error::TryRecvError::Disconnected) => {
                            info!("Input event channel disconnected");
                            return;
                        }
                    }
                }
                
                // Process the batch of events
                if !event_batch.is_empty() {
                    trace!("Processing {} input events", event_batch.len());
                    
                    for event in &event_batch {
                        stats.write().record_event(event);
                        
                        if config.log_events {
                            debug!("Processing input event: {:?}", event);
                        }
                        
                        // Process the event
                        match Self::process_single_event(&injector, event, &config).await {
                            Ok(()) => {
                                stats.write().record_success();
                                trace!("Successfully processed {} event", event.event_type());
                            }
                            Err(e) => {
                                error!("Failed to process input event: {}", e);
                                stats.write().record_failure(e.to_string());
                            }
                        }
                    }
                }
                
                // Log periodic statistics
                if stats_timer.elapsed().as_secs() >= INPUT_STATS_INTERVAL_SECONDS {
                    Self::log_stats(&stats).await;
                    stats_timer = Instant::now();
                }
            }
            
            info!("Input processing task ended for session {:?}", session_id);
        })
    }
    
    /// Process a single input event
    async fn process_single_event(
        injector: &Arc<RwLock<X11InputInjector>>,
        event: &InputEvent,
        config: &InputServiceConfig,
    ) -> Result<()> {
        match event {
            // Mouse events
            InputEvent::MouseMove { x, y, .. } => {
                if !config.enable_mouse { return Ok(()); }
                let mut inj = injector.write().await;
                inj.move_mouse_absolute(*x, *y)?;
            }
            
            InputEvent::MouseMoveRelative { dx, dy, .. } => {
                if !config.enable_mouse { return Ok(()); }
                let mut inj = injector.write().await;
                inj.move_mouse_relative(*dx, *dy)?;
            }
            
            InputEvent::MousePress { button, x, y, .. } => {
                if !config.enable_mouse { return Ok(()); }
                let mut inj = injector.write().await;
                inj.move_mouse_absolute(*x, *y)?;
                inj.press_mouse_button((*button).into())?;
            }
            
            InputEvent::MouseRelease { button, x, y, .. } => {
                if !config.enable_mouse { return Ok(()); }
                let mut inj = injector.write().await;
                inj.move_mouse_absolute(*x, *y)?;
                inj.release_mouse_button((*button).into())?;
            }
            
            InputEvent::MouseClick { button, x, y, double_click, .. } => {
                if !config.enable_mouse { return Ok(()); }
                let mut inj = injector.write().await;
                inj.move_mouse_absolute(*x, *y)?;
                
                if *double_click {
                    inj.double_click_mouse_button((*button).into())?;
                } else {
                    inj.click_mouse_button((*button).into())?;
                }
            }
            
            InputEvent::MouseScroll { direction, clicks, x, y, .. } => {
                if !config.enable_mouse { return Ok(()); }
                let mut inj = injector.write().await;
                inj.move_mouse_absolute(*x, *y)?;
                inj.scroll_mouse((*direction).into(), *clicks)?;
            }
            
            // Keyboard events
            InputEvent::KeyPress { key, modifiers, .. } => {
                if !config.enable_keyboard { return Ok(()); }
                let inj = injector.read().await;
                Self::process_key_press(&*inj, key, *modifiers)?;
            }
            
            InputEvent::KeyRelease { key, modifiers, .. } => {
                if !config.enable_keyboard { return Ok(()); }
                let inj = injector.read().await;
                Self::process_key_release(&*inj, key, *modifiers)?;
            }
            
            InputEvent::KeyStroke { key, modifiers, .. } => {
                if !config.enable_keyboard { return Ok(()); }
                let inj = injector.read().await;
                Self::process_key_stroke(&*inj, key, *modifiers)?;
            }
            
            InputEvent::TypeText { text, .. } => {
                if !config.enable_keyboard { return Ok(()); }
                let inj = injector.read().await;
                inj.type_string(text)?;
            }
            
            InputEvent::KeyCombination { keys, .. } => {
                if !config.enable_keyboard { return Ok(()); }
                let inj = injector.read().await;
                let converted_keys: Vec<_> = keys.iter().map(|k| (*k).into()).collect();
                inj.send_key_combination(&converted_keys)?;
            }
            
            // Clipboard events
            InputEvent::ClipboardSet { text, .. } => {
                if !config.enable_clipboard { return Ok(()); }
                // TODO: Implement clipboard setting
                debug!("Clipboard set requested: {} chars", text.len());
            }
            
            InputEvent::ClipboardGet { .. } => {
                if !config.enable_clipboard { return Ok(()); }
                // TODO: Implement clipboard getting
                debug!("Clipboard get requested");
            }
        }
        
        Ok(())
    }
    
    /// Process key press event
    fn process_key_press(
        injector: &X11InputInjector,
        key: &KeyType,
        modifiers: crate::input::input_protocol::ModifierFlags,
    ) -> Result<()> {
        match key {
            KeyType::Character(ch) => {
                let (keycode, key_modifiers) = injector.char_to_keycode(*ch)?;
                injector.press_key(keycode, key_modifiers)?;
            }
            KeyType::Keycode(code) => {
                injector.press_key(*code, modifiers.into())?;
            }
            KeyType::Special(special_key) => {
                let converted_key: crate::input::x11_input::SpecialKey = (*special_key).into();
                let keycode = converted_key.to_keycode();
                injector.press_key(keycode, modifiers.into())?;
            }
        }
        Ok(())
    }
    
    /// Process key release event
    fn process_key_release(
        injector: &X11InputInjector,
        key: &KeyType,
        modifiers: crate::input::input_protocol::ModifierFlags,
    ) -> Result<()> {
        match key {
            KeyType::Character(ch) => {
                let (keycode, key_modifiers) = injector.char_to_keycode(*ch)?;
                injector.release_key(keycode, key_modifiers)?;
            }
            KeyType::Keycode(code) => {
                injector.release_key(*code, modifiers.into())?;
            }
            KeyType::Special(special_key) => {
                let converted_key: crate::input::x11_input::SpecialKey = (*special_key).into();
                let keycode = converted_key.to_keycode();
                injector.release_key(keycode, modifiers.into())?;
            }
        }
        Ok(())
    }
    
    /// Process key stroke (press and release)
    fn process_key_stroke(
        injector: &X11InputInjector,
        key: &KeyType,
        modifiers: crate::input::input_protocol::ModifierFlags,
    ) -> Result<()> {
        match key {
            KeyType::Character(ch) => {
                let (keycode, key_modifiers) = injector.char_to_keycode(*ch)?;
                injector.press_and_release_key(keycode, key_modifiers)?;
            }
            KeyType::Keycode(code) => {
                injector.press_and_release_key(*code, modifiers.into())?;
            }
            KeyType::Special(special_key) => {
                let converted_key: crate::input::x11_input::SpecialKey = (*special_key).into();
                let keycode = converted_key.to_keycode();
                injector.press_and_release_key(keycode, modifiers.into())?;
            }
        }
        Ok(())
    }
    
    /// Check rate limiting
    async fn check_rate_limit(&self) -> bool {
        let stats = self.stats.read();
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Simple rate limiting based on events per second
        if stats.last_event_time > 0 {
            let time_diff = current_time - (stats.last_event_time / 1_000_000);
            if time_diff == 0 {
                // Check if we're exceeding the rate limit
                return stats.events_per_second(1) < self.config.max_events_per_second as f64;
            }
        }
        
        true // Allow event if we can't determine rate
    }
    
    /// Log periodic statistics
    async fn log_stats(stats: &Arc<ParkingRwLock<InputStats>>) {
        let stats_snapshot = stats.read().clone();
        info!("Input stats: {} events received, {} processed ({:.1}% success), {} mouse, {} keyboard",
            stats_snapshot.events_received,
            stats_snapshot.events_processed,
            stats_snapshot.success_rate(),
            stats_snapshot.mouse_events,
            stats_snapshot.keyboard_events
        );
        
        if !stats_snapshot.processing_errors.is_empty() {
            warn!("Recent input processing errors: {:?}", stats_snapshot.processing_errors);
        }
    }
    
    /// Log final statistics on shutdown
    async fn log_final_stats(&self) {
        let stats = self.stats.read().clone();
        info!("Final input service stats for session {:?}: {} total events, {:.1}% success rate",
            self.session_id,
            stats.events_received,
            stats.success_rate()
        );
    }
    
    /// Get current statistics
    pub fn get_stats(&self) -> InputStats {
        self.stats.read().clone()
    }
    
    /// Check if service is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }
    
    /// Get event sender for external use
    pub fn get_event_sender(&self) -> mpsc::Sender<InputEvent> {
        self.event_sender.clone()
    }
    
    /// Test input capabilities
    pub async fn test_input_capabilities(&self) -> Result<bool> {
        #[cfg(target_os = "linux")]
        {
            let injector = self.injector.read().await;
            injector.test_input_injection()
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            Ok(false)
        }
    }
}