//! RV8 Browser Main Entry Point
//!
//! Dispatches child process types. The `rv8` binary itself has no display
//! frontend or JavaScript engine; use `roverite` (`--features chrome`) or
//! `viewportd` (`--features servo-render`). Process types that are not
//! implemented exit with an error instead of idling forever.

use log::{error, info, warn};
use std::env;

use rv8::ipc::{self, IpcSender, RendererMessage};
use rv8::renderer;

fn main() {
    // Initialize logging with tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("rv8=info".parse().unwrap())
                .add_directive("wgpu=warn".parse().unwrap()),
        )
        .with_target(true)
        .with_thread_ids(true)
        .init();

    info!("rv8 v0.1");

    // Parse command line args
    let args: Vec<String> = env::args().collect();
    let initial_url = args.get(1).map(|s| s.as_str()).unwrap_or("about:blank");

    // Check for special process types (Chrome-like multi-process)
    if let Some(process_type) = args.iter().find(|a| a.starts_with("--type=")) {
        let ptype = process_type.strip_prefix("--type=").unwrap();
        match ptype {
            "renderer" => run_renderer_process(&args),
            "gpu" => run_gpu_process(&args),
            "network" => run_network_process(&args),
            "utility" => run_utility_process(&args),
            _ => {
                error!("Unknown process type: {}", ptype);
                std::process::exit(1);
            }
        }
        return;
    }

    // Main browser process
    run_browser_process(initial_url);
}

/// The default `rv8` binary has no window and no JavaScript engine, so it
/// refuses to start rather than parking a headless backend thread.
fn run_browser_process(initial_url: &str) {
    error!(
        "rv8 browser process is not implemented: this binary has no display frontend or \
         JavaScript engine (requested {initial_url}); run roverite (--features chrome) or \
         viewportd (--features servo-render)"
    );
    std::process::exit(1);
}

/// Run a renderer subprocess (not sandboxed; see `BrowserConfig::sandbox`)
fn run_renderer_process(args: &[String]) {
    info!("Starting renderer process...");

    // Extract IPC channel from args
    let channel_id = args
        .iter()
        .find(|a| a.starts_with("--channel-id="))
        .map(|a| a.strip_prefix("--channel-id=").unwrap())
        .expect("Renderer process requires --channel-id");

    info!("Renderer process connecting to channel: {}", channel_id);

    // Create renderer with IPC connection
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    rt.block_on(async {
        // Connect to bootstrap
        let bootstrap_tx = IpcSender::<IpcSender<RendererMessage>>::connect(channel_id.to_string())
            .expect("Failed to connect to bootstrap");

        // Create channel for me (Renderer -> Browser messages will use the browser_tx we receive later)
        // Wait, we need to create a channel for Receiving messages FROM Browser (RendererMessage).
        // And we send the Sender (to us) TO the browser.
        let (tx_to_me, rx_from_browser) =
            ipc::channel::<RendererMessage>().expect("Failed to create renderer channel");

        // Send my sender to browser so it can talk to me
        bootstrap_tx
            .send(tx_to_me)
            .expect("Failed to send sender to browser");

        // Wait for init message which contains the sender to browser
        let mut browser_tx_opt = None;
        let mut pending_messages = Vec::new();

        while let Ok(msg) = rx_from_browser.recv() {
            if let RendererMessage::Initialize { browser_tx } = msg {
                browser_tx_opt = Some(browser_tx);
                break;
            } else {
                warn!("Received unexpected message before Initialize: {:?}", msg);
                pending_messages.push(msg);
            }
        }

        let browser_tx = match browser_tx_opt {
            Some(tx) => tx,
            None => {
                error!("Failed to receive Initialize message before channel closed");
                return;
            }
        };

        // Bridge rx_from_browser (blocking IPC) to mpsc for tokio (async)
        let (mpsc_tx, mpsc_rx) = tokio::sync::mpsc::unbounded_channel();

        for msg in pending_messages {
            let _ = mpsc_tx.send(msg);
        }

        // Spawn bridge thread
        // Loop for the rest of messages
        rv8::ipc::bridge_ipc_receiver(rx_from_browser, mpsc_tx);

        let config = rv8::servo_embed::ServoConfig {
            user_agent: "RV8/0.1.0".to_string(),
            ..Default::default()
        };

        use std::process;
        let mut renderer = renderer::RendererProcess::new(process::id() as u64, browser_tx, config)
            .await
            .expect("Failed to create renderer process");

        renderer.run(mpsc_rx).await;
    });
}

/// Run the GPU process (compositing)
fn run_gpu_process(args: &[String]) {
    info!("Starting GPU process...");

    let channel_id = args
        .iter()
        .find(|a| a.starts_with("--channel-id="))
        .map(|a| a.strip_prefix("--channel-id=").unwrap())
        .expect("GPU process requires --channel-id");
    exit_unimplemented("GPU", channel_id);
}

/// Run the network process
fn run_network_process(args: &[String]) {
    info!("Starting network process...");

    let channel_id = args
        .iter()
        .find(|a| a.starts_with("--channel-id="))
        .map(|a| a.strip_prefix("--channel-id=").unwrap())
        .expect("Network process requires --channel-id");
    exit_unimplemented("Network", channel_id);
}

/// Run a utility process
fn run_utility_process(args: &[String]) {
    info!("Starting utility process...");

    let channel_id = args
        .iter()
        .find(|a| a.starts_with("--channel-id="))
        .map(|a| a.strip_prefix("--channel-id=").unwrap())
        .expect("Utility process requires --channel-id");
    exit_unimplemented("Utility", channel_id);
}

/// GPU, network and utility processes have no IPC bootstrap or work loop yet.
fn exit_unimplemented(kind: &str, channel_id: &str) -> ! {
    error!("{kind} process is not implemented (channel {channel_id})");
    std::process::exit(1);
}
