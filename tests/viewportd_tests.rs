use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

#[test]
fn test_viewportd_command_parsing() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_viewportd"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn viewportd process");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let mut stdout = child.stdout.take().expect("Failed to open stdout");
    let mut stderr = child.stderr.take().expect("Failed to open stderr");

    // Spawn a thread to feed stdin to avoid blocking reading stdout/stderr
    std::thread::spawn(move || {
        // Send valid commands
        let _ = stdin.write_all(b"NAV https://example.com\n");
        let _ = stdin.write_all(b"SIZE 800 600\n");
        let _ = stdin.write_all(b"SCROLL 0 100\n");
        let _ = stdin.write_all(b"FIND test FWD\n");
        let _ = stdin.write_all(b"FINDSTOP\n");
        let _ = stdin.write_all(b"CLICK 10 20\n");

        // Send invalid/malformed commands
        let _ = stdin.write_all(b"INVALID_CMD args\n");
        let _ = stdin.write_all(b"SIZE abc def\n");
        let _ = stdin.write_all(b"SCROLL abc\n");
        let _ = stdin.write_all(b"CLICK - -\n");

        // Let it process for a bit so it renders frames and produces stdout
        std::thread::sleep(Duration::from_millis(500));

        // Shut down cleanly
        let _ = stdin.write_all(b"QUIT\n");
    });

    let mut output_bytes = Vec::new();
    let _ = stdout.read_to_end(&mut output_bytes);

    let mut err_bytes = Vec::new();
    let _ = stderr.read_to_end(&mut err_bytes);

    let status = child.wait().expect("Failed to wait for child process");
    let code = status.code().unwrap_or(-1);

    // Either it succeeded (0) or failed to init headless display (1)
    // We want to ensure it didn't panic or crash due to our input
    assert!(
        code == 0 || code == 1,
        "viewportd exited with unexpected code: {}. stderr: {}",
        code,
        String::from_utf8_lossy(&err_bytes)
    );

    if code == 0 {
        // If it ran successfully, we expect it to have produced some frames or metadata
        // The magic markers are "RV8F" (frame), "RV8M" (meta), "RV8S" (find), etc.
        let has_magic = output_bytes.windows(4).any(|w| {
            w == b"RV8F" || w == b"RV8M" || w == b"RV8S" || w == b"RV8I" || w == b"RV8L"
        });

        assert!(
            has_magic,
            "viewportd exited successfully but produced no expected magic markers in stdout. Output length: {}",
            output_bytes.len()
        );
    }
}
