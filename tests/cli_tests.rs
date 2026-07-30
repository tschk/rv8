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
