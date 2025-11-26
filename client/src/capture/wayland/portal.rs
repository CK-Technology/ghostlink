//! XDG Desktop Portal integration for screen capture permissions
//!
//! Uses DBus to communicate with org.freedesktop.portal.ScreenCast
//! to request screen capture permissions and obtain PipeWire stream info.

use std::collections::HashMap;
use std::os::unix::io::{OwnedFd, FromRawFd};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dbus::arg::{PropMap, RefArg, Variant};
use dbus::blocking::{Proxy, SyncConnection};
use dbus::message::{MatchRule, MessageType};
use dbus::Message;

use tracing::{debug, info, warn};

use crate::error::{Result, GhostLinkError, CaptureError};

/// Portal session information
pub struct PortalSession {
    pub conn: Arc<SyncConnection>,
    pub session_path: dbus::Path<'static>,
    pub streams: Vec<StreamInfo>,
    pub fd: OwnedFd,
    pub supports_restore_token: bool,
}

impl std::fmt::Debug for PortalSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortalSession")
            .field("session_path", &self.session_path)
            .field("streams", &self.streams)
            .field("supports_restore_token", &self.supports_restore_token)
            .finish()
    }
}

/// Information about a PipeWire stream from the portal
#[derive(Debug, Clone, Copy)]
pub struct StreamInfo {
    pub path: u64,
    pub source_type: SourceType,
    pub position: (i32, i32),
    pub size: (u32, u32),
}

/// Type of capture source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    Monitor = 1,
    Window = 2,
    Virtual = 4,
}

impl From<u64> for SourceType {
    fn from(val: u64) -> Self {
        match val {
            1 => SourceType::Monitor,
            2 => SourceType::Window,
            4 => SourceType::Virtual,
            _ => SourceType::Monitor,
        }
    }
}

/// Portal response from DBus signal
#[derive(Debug)]
struct PortalResponse {
    response: u32,
    results: PropMap,
}

impl dbus::arg::ReadAll for PortalResponse {
    fn read(i: &mut dbus::arg::Iter) -> std::result::Result<Self, dbus::arg::TypeMismatchError> {
        Ok(PortalResponse {
            response: i.read()?,
            results: i.read()?,
        })
    }
}

/// ScreenCast portal interface
pub struct ScreenCastPortal {
    conn: SyncConnection,
    portal_version: u32,
}

impl ScreenCastPortal {
    /// Create a new ScreenCast portal connection
    pub fn new() -> Result<Self> {
        let conn = SyncConnection::new_session().map_err(|e| {
            GhostLinkError::Capture(CaptureError::InitializationFailed {
                reason: format!("DBus session connection failed: {}", e),
            })
        })?;

        let portal = Self::get_portal_proxy(&conn);

        // Get portal version to check feature support
        // Use introspection to get version property
        let version: u32 = match portal.method_call::<(dbus::arg::Variant<u32>,), _, _, _>(
            "org.freedesktop.DBus.Properties",
            "Get",
            ("org.freedesktop.portal.ScreenCast", "version"),
        ) {
            Ok((v,)) => v.0,
            Err(e) => {
                warn!("Failed to get portal version: {}, assuming v4", e);
                4 // Assume version 4 (restore token support)
            }
        };

        info!("ScreenCast portal version: {}", version);

        Ok(Self { conn, portal_version: version })
    }

    /// Get a proxy to the portal
    fn get_portal_proxy(conn: &SyncConnection) -> Proxy<&SyncConnection> {
        conn.with_proxy(
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            Duration::from_millis(5000),
        )
    }

    /// Check if restore tokens are supported (version >= 4)
    pub fn supports_restore_token(&self) -> bool {
        self.portal_version >= 4
    }

    /// Request screen capture access
    ///
    /// This will show a permission dialog to the user if needed.
    /// Returns a PortalSession with the PipeWire stream info.
    pub fn request_screen_capture(
        &self,
        capture_cursor: bool,
        restore_token: Option<&str>,
    ) -> Result<PortalSession> {
        let portal = Self::get_portal_proxy(&self.conn);

        // Shared state for async response handling
        let fd: Arc<Mutex<Option<OwnedFd>>> = Arc::new(Mutex::new(None));
        let streams: Arc<Mutex<Vec<StreamInfo>>> = Arc::new(Mutex::new(Vec::new()));
        let session_path: Arc<Mutex<Option<dbus::Path<'static>>>> = Arc::new(Mutex::new(None));
        let failure: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let new_restore_token: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        // Step 1: Create session
        let mut args: PropMap = HashMap::new();
        args.insert("session_handle_token".into(), Variant(Box::new("ghostlink_session".to_string())));
        args.insert("handle_token".into(), Variant(Box::new("ghostlink_handle".to_string())));

        let (create_result,): (dbus::Path<'static>,) = portal.method_call(
            "org.freedesktop.portal.ScreenCast",
            "CreateSession",
            (args,),
        ).map_err(|e| {
            GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: format!("CreateSession failed: {}", e),
            })
        })?;

        debug!("CreateSession request path: {}", create_result);

        // Set up response handler for CreateSession
        let fd_clone = fd.clone();
        let streams_clone = streams.clone();
        let session_clone = session_path.clone();
        let failure_clone = failure.clone();
        let restore_clone = new_restore_token.clone();
        let capture_cursor_flag = capture_cursor;
        let has_restore = restore_token.map(|s| s.to_string());
        let supports_restore = self.supports_restore_token();

        self.setup_response_handler(
            create_result.clone(),
            move |response, conn, _msg| {
                Self::on_create_session_response(
                    response,
                    conn,
                    fd_clone.clone(),
                    streams_clone.clone(),
                    session_clone.clone(),
                    failure_clone.clone(),
                    restore_clone.clone(),
                    capture_cursor_flag,
                    has_restore.clone(),
                    supports_restore,
                )
            },
            failure.clone(),
        )?;

        // Wait for user interaction (up to 3 minutes)
        for _ in 0..1800 {
            self.conn.process(Duration::from_millis(100)).map_err(|e| {
                GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                    reason: format!("DBus process failed: {}", e),
                })
            })?;

            // Check if we got the file descriptor
            if fd.lock().unwrap().is_some() {
                break;
            }

            // Check for failure
            if failure.load(Ordering::SeqCst) {
                return Err(GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                    reason: "Portal request failed or was cancelled by user".into(),
                }));
            }
        }

        // Extract results
        let fd_result = fd.lock().unwrap().take();
        let streams_result = streams.lock().unwrap().clone();
        let session_result = session_path.lock().unwrap().clone();

        match (fd_result, session_result) {
            (Some(fd), Some(session)) if !streams_result.is_empty() => {
                info!("Portal session established with {} streams", streams_result.len());
                Ok(PortalSession {
                    conn: Arc::new(SyncConnection::new_session().map_err(|e| {
                        GhostLinkError::Capture(CaptureError::InitializationFailed {
                            reason: format!("Failed to create new session connection: {}", e),
                        })
                    })?),
                    session_path: session,
                    streams: streams_result,
                    fd,
                    supports_restore_token: supports_restore,
                })
            }
            _ => Err(GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: "Failed to obtain screen capture session".into(),
            })),
        }
    }

    /// Set up a response handler for portal requests
    fn setup_response_handler<F>(
        &self,
        path: dbus::Path<'static>,
        handler: F,
        failure: Arc<AtomicBool>,
    ) -> Result<()>
    where
        F: FnMut(PortalResponse, &SyncConnection, &Message) -> Result<()> + Send + Sync + 'static,
    {
        let handler = Arc::new(Mutex::new(handler));

        let mut rule = MatchRule::new();
        rule.path = Some(path);
        rule.msg_type = Some(MessageType::Signal);
        rule.sender = Some("org.freedesktop.portal.Desktop".into());
        rule.interface = Some("org.freedesktop.portal.Request".into());

        self.conn.add_match(rule, move |response: PortalResponse, conn, msg| {
            debug!("Portal response: {:?}", response.response);

            match response.response {
                0 => {
                    // Success
                    if let Ok(mut handler) = handler.lock() {
                        if let Err(e) = handler(response, conn, msg) {
                            warn!("Response handler error: {}", e);
                            failure.store(true, Ordering::SeqCst);
                        }
                    }
                }
                1 => {
                    warn!("User cancelled the portal request");
                    failure.store(true, Ordering::SeqCst);
                }
                code => {
                    warn!("Portal request failed with code: {}", code);
                    failure.store(true, Ordering::SeqCst);
                }
            }
            true
        }).map_err(|e| {
            GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: format!("Failed to add DBus match: {}", e),
            })
        })?;

        Ok(())
    }

    /// Handle CreateSession response - proceed to SelectSources
    fn on_create_session_response(
        response: PortalResponse,
        conn: &SyncConnection,
        fd: Arc<Mutex<Option<OwnedFd>>>,
        streams: Arc<Mutex<Vec<StreamInfo>>>,
        session_path: Arc<Mutex<Option<dbus::Path<'static>>>>,
        failure: Arc<AtomicBool>,
        restore_token: Arc<Mutex<Option<String>>>,
        capture_cursor: bool,
        existing_restore_token: Option<String>,
        supports_restore: bool,
    ) -> Result<()> {
        // Extract session handle from response
        let session_handle: String = response.results
            .get("session_handle")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: "No session_handle in response".into(),
            }))?;

        debug!("Session created: {}", session_handle);
        *session_path.lock().unwrap() = Some(dbus::Path::from(session_handle.clone()));

        // Step 2: SelectSources
        let portal = conn.with_proxy(
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            Duration::from_millis(5000),
        );

        let mut args: PropMap = HashMap::new();
        args.insert("handle_token".into(), Variant(Box::new("ghostlink_sources".to_string())));
        // types: 1=monitor, 2=window, 4=virtual
        args.insert("types".into(), Variant(Box::new(1u32))); // Monitor only
        args.insert("multiple".into(), Variant(Box::new(true))); // Allow multiple monitors

        // Cursor mode: 1=hidden, 2=embedded, 4=metadata
        if capture_cursor {
            args.insert("cursor_mode".into(), Variant(Box::new(2u32))); // Embedded
        } else {
            args.insert("cursor_mode".into(), Variant(Box::new(1u32))); // Hidden
        }

        // Use restore token if available
        if supports_restore {
            if let Some(token) = existing_restore_token {
                args.insert("restore_token".into(), Variant(Box::new(token)));
            }
            args.insert("persist_mode".into(), Variant(Box::new(2u32))); // Persist until revoked
        }

        let (select_result,): (dbus::Path,) = portal.method_call(
            "org.freedesktop.portal.ScreenCast",
            "SelectSources",
            (dbus::Path::from(session_handle.clone()), args),
        ).map_err(|e| {
            GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: format!("SelectSources failed: {}", e),
            })
        })?;

        debug!("SelectSources request path: {}", select_result);

        // Set up handler for SelectSources response
        let session_handle_clone = session_handle.clone();
        Self::setup_select_sources_handler(
            conn,
            select_result,
            session_handle_clone,
            fd,
            streams,
            failure,
            restore_token,
        )?;

        Ok(())
    }

    /// Set up handler for SelectSources response - proceed to Start
    fn setup_select_sources_handler(
        conn: &SyncConnection,
        path: dbus::Path<'static>,
        session_handle: String,
        fd: Arc<Mutex<Option<OwnedFd>>>,
        streams: Arc<Mutex<Vec<StreamInfo>>>,
        failure: Arc<AtomicBool>,
        restore_token: Arc<Mutex<Option<String>>>,
    ) -> Result<()> {
        let mut rule = MatchRule::new();
        rule.path = Some(path);
        rule.msg_type = Some(MessageType::Signal);
        rule.sender = Some("org.freedesktop.portal.Desktop".into());
        rule.interface = Some("org.freedesktop.portal.Request".into());

        conn.add_match(rule, move |response: PortalResponse, conn, _msg| {
            if response.response != 0 {
                warn!("SelectSources failed with code: {}", response.response);
                failure.store(true, Ordering::SeqCst);
                return true;
            }

            debug!("SelectSources succeeded, calling Start");

            // Step 3: Start the stream
            let portal = conn.with_proxy(
                "org.freedesktop.portal.Desktop",
                "/org/freedesktop/portal/desktop",
                Duration::from_millis(5000),
            );

            let mut args: PropMap = HashMap::new();
            args.insert("handle_token".into(), Variant(Box::new("ghostlink_start".to_string())));

            let start_result: std::result::Result<(dbus::Path,), dbus::Error> = portal.method_call(
                "org.freedesktop.portal.ScreenCast",
                "Start",
                (dbus::Path::from(session_handle.clone()), "", args),
            );

            match start_result {
                Ok((start_path,)) => {
                    debug!("Start request path: {}", start_path);
                    let _ = Self::setup_start_handler(
                        conn,
                        start_path,
                        session_handle.clone(),
                        fd.clone(),
                        streams.clone(),
                        failure.clone(),
                        restore_token.clone(),
                    );
                }
                Err(e) => {
                    warn!("Start failed: {}", e);
                    failure.store(true, Ordering::SeqCst);
                }
            }

            true
        }).map_err(|e| {
            GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: format!("Failed to add SelectSources match: {}", e),
            })
        })?;

        Ok(())
    }

    /// Set up handler for Start response - extract streams and FD
    fn setup_start_handler(
        conn: &SyncConnection,
        path: dbus::Path<'static>,
        _session_handle: String,
        fd: Arc<Mutex<Option<OwnedFd>>>,
        streams: Arc<Mutex<Vec<StreamInfo>>>,
        failure: Arc<AtomicBool>,
        restore_token: Arc<Mutex<Option<String>>>,
    ) -> Result<()> {
        let mut rule = MatchRule::new();
        rule.path = Some(path);
        rule.msg_type = Some(MessageType::Signal);
        rule.sender = Some("org.freedesktop.portal.Desktop".into());
        rule.interface = Some("org.freedesktop.portal.Request".into());

        conn.add_match(rule, move |response: PortalResponse, conn, _msg| {
            if response.response != 0 {
                warn!("Start failed with code: {}", response.response);
                failure.store(true, Ordering::SeqCst);
                return true;
            }

            debug!("Start succeeded, extracting streams");

            // Extract restore token if present
            if let Some(token) = response.results.get("restore_token") {
                if let Some(token_str) = token.as_str() {
                    *restore_token.lock().unwrap() = Some(token_str.to_string());
                    debug!("Got restore token for future use");
                }
            }

            // Extract streams
            let extracted_streams = Self::extract_streams_from_response(&response);
            if extracted_streams.is_empty() {
                warn!("No streams in Start response");
                failure.store(true, Ordering::SeqCst);
                return true;
            }

            *streams.lock().unwrap() = extracted_streams;

            // Get PipeWire file descriptor
            let portal = conn.with_proxy(
                "org.freedesktop.portal.Desktop",
                "/org/freedesktop/portal/desktop",
                Duration::from_millis(5000),
            );

            // OpenPipeWireRemote returns a file descriptor
            let pw_fd_result: std::result::Result<(std::os::unix::io::RawFd,), dbus::Error> = portal.method_call(
                "org.freedesktop.portal.ScreenCast",
                "OpenPipeWireRemote",
                (dbus::Path::from("/org/freedesktop/portal/desktop"), HashMap::<String, Variant<Box<dyn RefArg>>>::new()),
            );

            match pw_fd_result {
                Ok((raw_fd,)) => {
                    debug!("Got PipeWire FD: {}", raw_fd);
                    // Safety: We trust the portal to give us a valid FD
                    let owned_fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
                    *fd.lock().unwrap() = Some(owned_fd);
                }
                Err(e) => {
                    warn!("OpenPipeWireRemote failed: {}", e);
                    failure.store(true, Ordering::SeqCst);
                }
            }

            true
        }).map_err(|e| {
            GhostLinkError::Capture(CaptureError::FrameCaptureFailed {
                reason: format!("Failed to add Start match: {}", e),
            })
        })?;

        Ok(())
    }

    /// Extract stream information from portal response
    fn extract_streams_from_response(response: &PortalResponse) -> Vec<StreamInfo> {
        let streams_variant = match response.results.get("streams") {
            Some(v) => v,
            None => return Vec::new(),
        };

        let mut result = Vec::new();

        // Parse the complex nested structure
        if let Some(iter) = streams_variant.as_iter() {
            for item in iter {
                if let Some(inner_iter) = item.as_iter() {
                    for stream in inner_iter {
                        if let Some(mut stream_iter) = stream.as_iter() {
                            // First element is path (u64)
                            let path = stream_iter.next()
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);

                            // Second element is properties dict
                            let mut source_type = SourceType::Monitor;
                            let mut position = (0i32, 0i32);
                            let mut size = (0u32, 0u32);

                            if let Some(props) = stream_iter.next() {
                                if let Some(props_iter) = props.as_iter() {
                                    let items: Vec<_> = props_iter.collect();
                                    // Properties are key-value pairs
                                    for chunk in items.chunks(2) {
                                        if chunk.len() == 2 {
                                            if let Some(key) = chunk[0].as_str() {
                                                match key {
                                                    "source_type" => {
                                                        if let Some(v) = chunk[1].as_u64() {
                                                            source_type = SourceType::from(v);
                                                        }
                                                    }
                                                    "size" => {
                                                        // Parse (width, height) tuple
                                                        if let Some(size_iter) = chunk[1].as_iter() {
                                                            let vals: Vec<i64> = size_iter
                                                                .flat_map(|v| v.as_iter())
                                                                .flatten()
                                                                .filter_map(|v| v.as_i64())
                                                                .collect();
                                                            if vals.len() >= 2 {
                                                                size = (vals[0] as u32, vals[1] as u32);
                                                            }
                                                        }
                                                    }
                                                    "position" => {
                                                        // Parse (x, y) tuple
                                                        if let Some(pos_iter) = chunk[1].as_iter() {
                                                            let vals: Vec<i64> = pos_iter
                                                                .flat_map(|v| v.as_iter())
                                                                .flatten()
                                                                .filter_map(|v| v.as_i64())
                                                                .collect();
                                                            if vals.len() >= 2 {
                                                                position = (vals[0] as i32, vals[1] as i32);
                                                            }
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            if path > 0 {
                                result.push(StreamInfo {
                                    path,
                                    source_type,
                                    position,
                                    size,
                                });
                                debug!("Found stream: path={}, size={:?}, pos={:?}", path, size, position);
                            }
                        }
                    }
                }
            }
        }

        result
    }
}
