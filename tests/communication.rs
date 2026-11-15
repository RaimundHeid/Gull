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
