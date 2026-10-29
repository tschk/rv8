use std::process::Command;

#[test]
fn test_unknown_process_type() {
    let output = Command::new(env!("CARGO_BIN_EXE_rv8"))
        .arg("--type=unknown")
        .output()
        .expect("Failed to execute process");

    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stderr.contains("Unknown process type: unknown") || stdout.contains("Unknown process type: unknown"),
        "Expected error message not found. stdout: {}, stderr: {}", stdout, stderr
    );
}

#[test]
fn test_renderer_process_requires_channel_id() {
    let output = Command::new(env!("CARGO_BIN_EXE_rv8"))
        .arg("--type=renderer")
        .output()
        .expect("Failed to execute process");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stderr.contains("Renderer process requires --channel-id") || stdout.contains("Renderer process requires --channel-id"),
        "Expected panic message not found. stdout: {}, stderr: {}", stdout, stderr
    );
}

#[test]
fn test_gpu_process_requires_channel_id() {
    let output = Command::new(env!("CARGO_BIN_EXE_rv8"))
        .arg("--type=gpu")
        .output()
        .expect("Failed to execute process");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stderr.contains("GPU process requires --channel-id") || stdout.contains("GPU process requires --channel-id"),
        "Expected panic message not found. stdout: {}, stderr: {}", stdout, stderr
    );
}

#[test]
fn test_network_process_requires_channel_id() {
    let output = Command::new(env!("CARGO_BIN_EXE_rv8"))
        .arg("--type=network")
        .output()
        .expect("Failed to execute process");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stderr.contains("Network process requires --channel-id") || stdout.contains("Network process requires --channel-id"),
        "Expected panic message not found. stdout: {}, stderr: {}", stdout, stderr
    );
}

#[test]
fn test_utility_process_requires_channel_id() {
    let output = Command::new(env!("CARGO_BIN_EXE_rv8"))
        .arg("--type=utility")
        .output()
        .expect("Failed to execute process");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stderr.contains("Utility process requires --channel-id") || stdout.contains("Utility process requires --channel-id"),
        "Expected panic message not found. stdout: {}, stderr: {}", stdout, stderr
    );
}

/// Runs rv8 and fails the test if it is still running after 10 s, which is
/// how the old parked/pending process types behaved.
fn run_with_deadline(args: &[&str]) -> (bool, String) {
    use std::io::Read;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let mut child = Command::new(env!("CARGO_BIN_EXE_rv8"))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to execute process");
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > Duration::from_secs(10) {
            child.kill().unwrap();
            panic!("rv8 {args:?} was still running after 10 s");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut output = String::new();
    child.stdout.take().unwrap().read_to_string(&mut output).unwrap();
    child.stderr.take().unwrap().read_to_string(&mut output).unwrap();
    (status.success(), output)
}

#[test]
fn test_default_build_refuses_to_start_without_frontend() {
    let (success, output) = run_with_deadline(&["https://example.com"]);
    assert!(!success, "{output}");
    assert!(output.contains("browser process is not implemented"), "{output}");
}

#[test]
fn test_unimplemented_process_types_exit_with_error() {
    for (kind, message) in [
        ("gpu", "GPU process is not implemented"),
        ("network", "Network process is not implemented"),
        ("utility", "Utility process is not implemented"),
    ] {
        let (success, output) =
            run_with_deadline(&[&format!("--type={kind}"), "--channel-id=test"]);
        assert!(!success, "{kind}: {output}");
        assert!(output.contains(message), "{kind}: {output}");
    }
}
