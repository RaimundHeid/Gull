//! Exercises the UCI front end as a real process.

use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn uci_process_reports_identity_and_bestmove() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gull"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn gull");
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "uci").unwrap();
        writeln!(stdin, "isready").unwrap();
        writeln!(stdin, "position startpos moves e2e4 e7e5").unwrap();
        writeln!(stdin, "go depth 2").unwrap();
        writeln!(stdin, "quit").unwrap();
    }
    let output = child.wait_with_output().expect("wait");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("uciok"), "{text}");
    assert!(text.contains("readyok"), "{text}");
    assert!(text.contains("bestmove"), "{text}");
    assert!(
        text.lines()
            .any(|l| l.starts_with("bestmove ") && l.len() >= 12),
        "{text}"
    );
}

#[test]
fn uci_applies_a_legal_move_missed_by_the_old_validator() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gull"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn gull");
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "uci").unwrap();
        writeln!(
            stdin,
            "position startpos moves h2h3 g8f6 g1f3 e7e6 e2e4 f6e4"
        )
        .unwrap();
        writeln!(stdin, "d").unwrap();
        writeln!(stdin, "quit").unwrap();
    }
    let output = child.wait_with_output().expect("wait");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("rnbqkb1r/pppp1ppp/4p3/8/4n3/5N1P/PPPP1PP1/RNBQKB1R w"),
        "{text}"
    );
}

#[test]
fn uci_threads_option_and_smp_search() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gull"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn gull");
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "uci").unwrap();
        writeln!(stdin, "setoption name Threads value 4").unwrap();
        writeln!(stdin, "isready").unwrap();
        writeln!(stdin, "position startpos moves e2e4 e7e5").unwrap();
        writeln!(stdin, "go depth 5").unwrap();
        writeln!(stdin, "quit").unwrap();
    }
    let output = child.wait_with_output().expect("wait");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("max 256"), "{text}");
    assert!(text.contains("bestmove"), "{text}");
}

#[test]
fn uci_smp_start_stop() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gull"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn gull");
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "uci").unwrap();
        writeln!(stdin, "setoption name Threads value 8").unwrap();
        writeln!(stdin, "position startpos").unwrap();
        writeln!(stdin, "go infinite").unwrap();
    }
    std::thread::sleep(std::time::Duration::from_millis(150));
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "stop").unwrap();
        writeln!(stdin, "quit").unwrap();
    }
    let output = child.wait_with_output().expect("wait");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("bestmove"), "{text}");
}

#[test]
fn uci_thread_change_restarts_running_analysis() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gull"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn gull");
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "uci").unwrap();
        writeln!(stdin, "setoption name Threads value 1").unwrap();
        writeln!(stdin, "position startpos").unwrap();
        writeln!(stdin, "go infinite").unwrap();
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "setoption name Threads value 4").unwrap();
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "stop").unwrap();
        writeln!(stdin, "quit").unwrap();
    }
    let output = child.wait_with_output().expect("wait");
    let text = String::from_utf8_lossy(&output.stdout);
    // One bestmove from stopping the old analysis and one from the restarted
    // 4-thread analysis.
    assert!(
        text.lines().filter(|l| l.starts_with("bestmove")).count() >= 2,
        "{text}"
    );
}
