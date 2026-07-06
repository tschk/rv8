//! Process manager for Chrome-like multi-process architecture

#[cfg(target_os = "linux")]
use log::warn;
use log::{debug, info};
#[cfg(target_os = "linux")]
use nix::sched::sched_setaffinity;
#[cfg(target_os = "linux")]
use nix::sched::CpuSet;
use std::collections::HashMap;
use std::process::{Child, Command};
use tokio::sync::Mutex;

use super::TabId;
use crate::ipc::{self, BrowserMessage, IpcServer, RendererClient, RendererMessage};
#[cfg(not(feature = "servo-render"))]
use crate::renderer::RendererProcess;
#[cfg(not(feature = "servo-render"))]
use crate::servo_embed::ServoConfig;

/// Process manager for spawning and managing child processes
pub struct ProcessManager {
    /// Whether multi-process is enabled
    multi_process: bool,

    /// Active renderer processes (tab_id -> child process)
    renderers: Mutex<HashMap<TabId, ChildProcess>>,

    /// GPU process
    gpu_process: Mutex<Option<ChildProcess>>,

    /// Network process
    network_process: Mutex<Option<ChildProcess>>,

    /// IPC server for child process communication
    ipc_server: IpcServer,

    /// Core assignment counter for per-tab isolation (Linux CPU pinning)
    #[cfg(target_os = "linux")]
    core_counter: std::sync::atomic::AtomicUsize,

    /// Optional forwarder for renderer → browser messages
    browser_events: Mutex<Option<tokio::sync::mpsc::UnboundedSender<BrowserMessage>>>,
}

/// Wrapper for child process
struct ChildProcess {
    _child: Child,
    channel_id: String,
}

impl ProcessManager {
    /// Create process manager for multi-process mode
    pub fn new_multi_process() -> Self {
        info!("Creating multi-process manager");
        ProcessManager {
            multi_process: true,
            renderers: Mutex::new(HashMap::new()),
            gpu_process: Mutex::new(None),
            network_process: Mutex::new(None),
            ipc_server: IpcServer::new(),
            #[cfg(target_os = "linux")]
            core_counter: std::sync::atomic::AtomicUsize::new(0),
            browser_events: Mutex::new(None),
        }
    }

    /// Create process manager for single-process mode
    pub fn new_single_process() -> Self {
        info!("Creating single-process manager");
        ProcessManager {
            multi_process: false,
            renderers: Mutex::new(HashMap::new()),
            gpu_process: Mutex::new(None),
            network_process: Mutex::new(None),
            ipc_server: IpcServer::new(),
            #[cfg(target_os = "linux")]
            core_counter: std::sync::atomic::AtomicUsize::new(0),
            browser_events: Mutex::new(None),
        }
    }

    pub async fn set_browser_event_forwarder(
        &self,
        tx: tokio::sync::mpsc::UnboundedSender<BrowserMessage>,
    ) {
        *self.browser_events.lock().await = Some(tx);
    }

    /// Spawn a renderer process for a tab
    pub async fn spawn_renderer(&self, tab_id: TabId) -> Result<RendererClient, String> {
        if self.multi_process {
            self.spawn_renderer_process(tab_id).await
        } else {
            // Single process mode - create in-process channel
            self.create_inprocess_renderer(tab_id).await
        }
    }

    /// Spawn an actual renderer subprocess
    async fn spawn_renderer_process(&self, tab_id: TabId) -> Result<RendererClient, String> {
        // 1. Create a bootstrap server
        let (server_name, bootstrap) = IpcServer::create_bootstrap_server()?;

        info!(
            "Spawning renderer process for tab {} with bootstrap {}",
            tab_id.0, server_name
        );

        // 2. Spawn child process
        let exe =
            std::env::current_exe().map_err(|e| format!("Failed to get current exe: {}", e))?;

        if !server_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err("Invalid server name".into());
        }

        // We pass the bootstrap server name via --channel-id as expected by main.rs
        let child = Command::new(exe)
            .arg("--type=renderer")
            .arg(format!("--channel-id={}", server_name))
            .arg(format!("--tab-id={}", tab_id.0))
            .spawn()
            .map_err(|e| format!("Failed to spawn renderer: {}", e))?;

        // Set CPU affinity for per-tab core isolation
        #[cfg(target_os = "linux")]
        {
            let core_count = num_cpus::get();
            let core = self
                .core_counter
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                % core_count;
            let pid = nix::unistd::Pid::from_raw(child.id() as i32);
            let mut cpuset = CpuSet::new();
            if let Err(e) = cpuset.set(core) {
                warn!(
                    "Failed to add core {} to cpuset for renderer {}: {}",
                    core, tab_id.0, e
                );
            } else if let Err(e) = sched_setaffinity(pid, &cpuset) {
                warn!(
                    "Failed to set CPU affinity for renderer {} to core {}: {}",
                    tab_id.0, core, e
                );
            }
        }

        // 3. Accept connection (handshake)
        // Run blocking accept in a blocking task with a timeout
        let (_, tx_to_renderer) = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::task::spawn_blocking(move || {
                bootstrap
                    .accept()
                    .map_err(|e| format!("Failed to accept connection from renderer: {}", e))
            }),
        )
        .await
        .map_err(|_| "Timeout waiting for renderer connection".to_string())?
        .map_err(|e| format!("Join error: {}", e))??;

        // 4. Create channel for receiving messages from renderer (BrowserMessage)
        // tx_to_browser (sent to renderer), rx_from_renderer (kept by browser)
        let (tx_to_browser, rx_from_renderer) =
            ipc::channel::<BrowserMessage>().map_err(|e| e.to_string())?;

        // 5. Send Initialize message to renderer with tx_to_browser
        tx_to_renderer
            .send(RendererMessage::Initialize {
                browser_tx: tx_to_browser,
            })
            .map_err(|e| e.to_string())?;

        // 6. Handle rx_from_renderer
        let event_tx = self.browser_events.lock().await.clone();
        let crashed_tab_id = tab_id.0;
        std::thread::spawn(move || {
            while let Ok(msg) = rx_from_renderer.recv() {
                if let Some(ref tx) = event_tx {
                    let _ = tx.send(msg);
                }
            }
            // Channel closed — renderer exited or crashed
            if let Some(ref tx) = event_tx {
                let _ = tx.send(BrowserMessage::RendererCrashed {
                    tab_id: crashed_tab_id,
                });
            }
        });

        // Store child process
        {
            let mut renderers = self.renderers.lock().await;
            renderers.insert(
                tab_id,
                ChildProcess {
                    _child: child,
                    channel_id: server_name.clone(),
                },
            );
        }

        Ok(RendererClient::new(tab_id.0, tx_to_renderer))
    }

    /// Create in-process renderer (single-process mode)
    async fn create_inprocess_renderer(&self, tab_id: TabId) -> Result<RendererClient, String> {
        debug!("Creating in-process renderer for tab {}", tab_id.0);

        // Channel 1: Browser -> Renderer (RendererMessage)
        let (tx_to_renderer, rx_from_browser) =
            ipc::channel::<RendererMessage>().map_err(|e| e.to_string())?;

        // Channel 2: Renderer -> Browser (BrowserMessage)
        let (tx_to_browser, rx_from_renderer) =
            ipc::channel::<BrowserMessage>().map_err(|e| e.to_string())?;

        // Bridge: Renderer -> Browser messages into event loop
        let event_tx = self.browser_events.lock().await.clone();
        let crashed_tab_id = tab_id.0;
        std::thread::spawn(move || {
            while let Ok(msg) = rx_from_renderer.recv() {
                if let Some(ref tx) = event_tx {
                    let _ = tx.send(msg);
                }
            }
            // Channel closed — renderer exited or crashed
            if let Some(ref tx) = event_tx {
                let _ = tx.send(BrowserMessage::RendererCrashed {
                    tab_id: crashed_tab_id,
                });
            }
        });

        // Bridge: Browser -> Renderer messages into mpsc for async
        let (mpsc_tx, mpsc_rx) = tokio::sync::mpsc::unbounded_channel();
        crate::ipc::bridge_ipc_receiver(rx_from_browser, mpsc_tx);

        // Start the renderer in-process
        // ponytail: skip for servo-render — Servo's global opts init conflicts with
        // parallel use-Servo tests. Single-process + servo-render uses multi-process
        // bootstrap instead for real rendering.
        #[cfg(not(feature = "servo-render"))]
        {
            let config = ServoConfig::default();
            let tab_id_val = tab_id.0;
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("renderer tokio runtime");
                rt.block_on(async {
                    match RendererProcess::new(tab_id_val, tx_to_browser, config).await {
                        Ok(mut renderer) => renderer.run(mpsc_rx).await,
                        Err(e) => log::error!(
                            "Failed to create in-process renderer for tab {}: {}",
                            tab_id_val,
                            e
                        ),
                    }
                });
            });
        }
        #[cfg(feature = "servo-render")]
        {
            let _ = tx_to_browser;
            let _ = mpsc_rx;
        }

        Ok(RendererClient::new(tab_id.0, tx_to_renderer))
    }

    /// Terminate a renderer process
    pub async fn terminate_renderer(&self, tab_id: TabId) {
        let mut renderers = self.renderers.lock().await;
        if let Some(mut process) = renderers.remove(&tab_id) {
            info!("Terminating renderer for tab {}", tab_id.0);
            let _ = process._child.kill();
            self.ipc_server.close_channel(&process.channel_id).await;
        }
    }

    /// Spawn GPU process (if not already running)
    pub async fn ensure_gpu_process(&self) -> Result<(), String> {
        if !self.multi_process {
            return Ok(());
        }

        let mut gpu = self.gpu_process.lock().await;
        if gpu.is_some() {
            return Ok(());
        }

        let channel_id = "gpu-process";
        info!("Spawning GPU process with channel {}", channel_id);

        let (_channel, _rx) = self.ipc_server.create_channel(channel_id)?;

        let exe =
            std::env::current_exe().map_err(|e| format!("Failed to get current exe: {}", e))?;

        if !channel_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err("Invalid channel id".into());
        }

        let child = Command::new(exe)
            .arg("--type=gpu")
            .arg(format!("--channel-id={}", channel_id))
            .spawn()
            .map_err(|e| format!("Failed to spawn GPU process: {}", e))?;

        *gpu = Some(ChildProcess {
            _child: child,
            channel_id: channel_id.to_string(),
        });

        Ok(())
    }

    /// Spawn network process (if not already running)
    pub async fn ensure_network_process(&self) -> Result<(), String> {
        if !self.multi_process {
            return Ok(());
        }

        let mut network = self.network_process.lock().await;
        if network.is_some() {
            return Ok(());
        }

        let channel_id = "network-process";
        info!("Spawning network process with channel {}", channel_id);

        let (_channel, _rx) = self.ipc_server.create_channel(channel_id)?;

        let exe =
            std::env::current_exe().map_err(|e| format!("Failed to get current exe: {}", e))?;

        if !channel_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err("Invalid channel id".into());
        }

        let child = Command::new(exe)
            .arg("--type=network")
            .arg(format!("--channel-id={}", channel_id))
            .spawn()
            .map_err(|e| format!("Failed to spawn network process: {}", e))?;

        *network = Some(ChildProcess {
            _child: child,
            channel_id: channel_id.to_string(),
        });

        Ok(())
    }

    /// Shutdown all processes
    pub async fn shutdown(&self) {
        info!("Shutting down all child processes");

        // Terminate all renderers
        let mut renderers = self.renderers.lock().await;
        for (tab_id, mut process) in renderers.drain() {
            debug!("Terminating renderer for tab {}", tab_id.0);
            let _ = process._child.kill();
        }

        // Terminate GPU process
        if let Some(mut process) = self.gpu_process.lock().await.take() {
            debug!("Terminating GPU process");
            let _ = process._child.kill();
        }

        // Terminate network process
        if let Some(mut process) = self.network_process.lock().await.take() {
            debug!("Terminating network process");
            let _ = process._child.kill();
        }
    }
}
