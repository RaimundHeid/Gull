//! UCI protocol layer.
//!
//! This is deliberately small: it parses the UCI commands needed to drive the
//! engine and prints the standard `info` / `bestmove` responses.  The search
//! itself is single-threaded; `go` runs it in a worker thread so that `stop`
//! and `quit` remain responsive.

use std::io::{self, BufRead, Write};
use std::thread::{self, JoinHandle};

use crate::position::Engine;
use crate::search::{Info, Reporter, SearchControl, SearchLimits, SearchResult};
use crate::types::{move_to_string, uci_score};

const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// Search output sink for UCI.
pub struct UciReporter;

impl Reporter for UciReporter {
    fn info_depth(&mut self, depth: i32) {
        println!("info depth {depth}");
        let _ = io::stdout().flush();
    }

    fn pv(&mut self, info: &Info) {
        let (kind, value) = uci_score(info.score);
        let mut pv = String::new();
        for (i, &mv) in info.pv.iter().enumerate() {
            if mv == 0 {
                break;
            }
            if i != 0 {
                pv.push(' ');
            }
            pv.push_str(&move_to_string(mv));
        }
        let nps = if info.time_ms == 0 {
            info.nodes.saturating_mul(1000)
        } else {
            info.nodes.saturating_mul(1000) / info.time_ms
        };
        println!(
            "info depth {} seldepth {} score {} {} nodes {} nps {} time {} pv {}",
            info.depth, info.sel_depth, kind, value, info.nodes, nps, info.time_ms, pv
        );
        let _ = io::stdout().flush();
    }
}

struct Running {
    handle: JoinHandle<(Engine, SearchResult)>,
    control: SearchControl,
}

/// Run the UCI command loop on stdin/stdout.
pub fn run() {
    let mut engine = Some(Engine::new());
    engine.as_mut().unwrap().reporter = Box::new(UciReporter);
    let mut running: Option<Running> = None;
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    loop {
        let line = match lines.next() {
            Some(Ok(line)) => line,
            Some(Err(_)) | None => break,
        };
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }
        match tokens[0] {
            "uci" => {
                println!("id name Gull");
                println!("id author Gull contributors");
                println!("option name Hash type spin default 128 min 1 max 32768");
                println!("option name Threads type spin default 1 min 1 max 1");
                println!("uciok");
            }
            "isready" => println!("readyok"),
            "setoption" => {
                if let Some((name, value)) = parse_option(&tokens) {
                    if name.eq_ignore_ascii_case("Hash") {
                        stop_and_join(&mut running, &mut engine);
                        if let (Some(e), Ok(mb)) = (engine.as_mut(), value.parse::<u64>()) {
                            e.resize_hash(mb);
                        }
                    }
                }
            }
            "ucinewgame" => {
                stop_and_join(&mut running, &mut engine);
                if let Some(e) = engine.as_mut() {
                    e.init_search(true);
                }
            }
            "position" => {
                stop_and_join(&mut running, &mut engine);
                if let Some(e) = engine.as_mut() {
                    let _ = set_position(e, &tokens[1..]);
                }
            }
            "go" => {
                stop_and_join(&mut running, &mut engine);
                let limits = parse_go(&tokens[1..]);
                let mut moved = engine.take().unwrap();
                moved.control.reset();
                let control = moved.control.clone();
                let handle = thread::spawn(move || {
                    let result = moved.go(&limits);
                    print_bestmove(&result);
                    (moved, result)
                });
                running = Some(Running { handle, control });
            }
            "stop" => {
                if let Some(r) = running.as_ref() {
                    r.control.request_stop();
                }
                stop_and_join(&mut running, &mut engine);
            }
            "ponderhit" => {}
            "quit" => {
                if let Some(r) = running.as_ref() {
                    r.control.request_stop();
                }
                stop_and_join(&mut running, &mut engine);
                break;
            }
            "d" | "print" => {
                if running.is_none() {
                    if let Some(e) = engine.as_ref() {
                        println!("Fen: {}", e.current_fen());
                    }
                }
            }
            _ => {}
        }
        let _ = io::stdout().flush();
    }
}

fn stop_and_join(running: &mut Option<Running>, engine: &mut Option<Engine>) {
    if let Some(r) = running.take() {
        r.control.request_stop();
        match r.handle.join() {
            Ok((e, _)) => *engine = Some(e),
            Err(_) => {
                // The search normally does not panic.  If it did, keep the
                // previous engine object so the session can continue.
            }
        }
    }
}

fn print_bestmove(result: &SearchResult) {
    if result.best_move == 0 {
        println!("bestmove 0000");
        return;
    }
    let best = move_to_string(result.best_move);
    if result.ponder_move != 0 {
        println!(
            "bestmove {} ponder {}",
            best,
            move_to_string(result.ponder_move)
        );
    } else {
        println!("bestmove {best}");
    }
    let _ = io::stdout().flush();
}

fn parse_option<'a>(tokens: &'a [&'a str]) -> Option<(String, &'a str)> {
    let name_pos = tokens.iter().position(|t| t.eq_ignore_ascii_case("name"))?;
    let value_pos = tokens
        .iter()
        .position(|t| t.eq_ignore_ascii_case("value"))?;
    if value_pos <= name_pos + 1 {
        return None;
    }
    let name = tokens[name_pos + 1..value_pos].join(" ");
    let value = tokens.get(value_pos + 1).copied().unwrap_or("");
    Some((name, value))
}

fn set_position(engine: &mut Engine, tokens: &[&str]) -> bool {
    if tokens.is_empty() {
        return false;
    }
    let mut i;
    match tokens[0] {
        "startpos" => {
            engine.get_board(START_FEN);
            i = 1;
        }
        "fen" => {
            let mut fields = Vec::new();
            i = 1;
            while i < tokens.len() && tokens[i] != "moves" && fields.len() < 6 {
                fields.push(tokens[i]);
                i += 1;
            }
            engine.get_board(&fields.join(" "));
        }
        _ => {
            return false;
        }
    }
    if i < tokens.len() && tokens[i] == "moves" {
        for mv in &tokens[i + 1..] {
            if !engine.play_uci_move(mv) {
                // Never continue from a partially applied position: doing so
                // would desynchronise the engine from the GUI.
                return false;
            }
        }
    }
    true
}

fn parse_go(tokens: &[&str]) -> SearchLimits {
    let mut limits = SearchLimits::default();
    limits.infinite = false;
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "depth" if i + 1 < tokens.len() => {
                limits.depth = tokens[i + 1].parse().unwrap_or(limits.depth);
                i += 1;
            }
            "movetime" if i + 1 < tokens.len() => {
                limits.movetime = tokens[i + 1].parse().ok();
                i += 1;
            }
            "nodes" if i + 1 < tokens.len() => {
                limits.nodes = tokens[i + 1].parse().unwrap_or(0);
                i += 1;
            }
            "wtime" if i + 1 < tokens.len() => {
                limits.wtime = tokens[i + 1].parse().unwrap_or(0);
                i += 1;
            }
            "btime" if i + 1 < tokens.len() => {
                limits.btime = tokens[i + 1].parse().unwrap_or(0);
                i += 1;
            }
            "winc" if i + 1 < tokens.len() => {
                limits.winc = tokens[i + 1].parse().unwrap_or(0);
                i += 1;
            }
            "binc" if i + 1 < tokens.len() => {
                limits.binc = tokens[i + 1].parse().unwrap_or(0);
                i += 1;
            }
            "movestogo" if i + 1 < tokens.len() => {
                limits.movestogo = tokens[i + 1].parse().unwrap_or(0);
                i += 1;
            }
            "infinite" => limits.infinite = true,
            "ponder" => limits.ponder = true,
            _ => {}
        }
        i += 1;
    }
    limits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_go_limits() {
        let l = parse_go(&[
            "wtime",
            "60000",
            "btime",
            "30000",
            "winc",
            "1000",
            "binc",
            "500",
            "movestogo",
            "20",
            "depth",
            "12",
            "nodes",
            "500000",
            "infinite",
        ]);
        assert_eq!(l.wtime, 60000);
        assert_eq!(l.btime, 30000);
        assert_eq!(l.winc, 1000);
        assert_eq!(l.binc, 500);
        assert_eq!(l.movestogo, 20);
        assert_eq!(l.depth, 12);
        assert_eq!(l.nodes, 500000);
        assert!(l.infinite);
        let l = parse_go(&["movetime", "250"]);
        assert_eq!(l.movetime, Some(250));
    }

    #[test]
    fn parses_setoption_name_and_value() {
        let (name, value) = parse_option(&["setoption", "name", "Hash", "value", "64"]).unwrap();
        assert_eq!(name, "Hash");
        assert_eq!(value, "64");
    }

    #[test]
    fn applies_uci_position_and_moves() {
        let mut e = Box::new(Engine::with_hash_mb(1));
        set_position(
            &mut e,
            &["startpos", "moves", "e2e4", "e7e5", "g1f3", "b8c6"],
        );
        let direct = "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3";
        let mut d = Box::new(Engine::with_hash_mb(1));
        d.get_board(direct);
        assert_eq!(e.pos.cur().key, d.pos.cur().key);
        assert_eq!(e.pos.cur().pawn_key, d.pos.cur().pawn_key);
        assert_eq!(e.pos.cur().turn, d.pos.cur().turn);
    }

    #[test]
    fn plays_promotion_castling_and_en_passant() {
        let mut e = Box::new(Engine::with_hash_mb(1));
        e.get_board("4k3/P7/8/8/8/8/8/4K3 w - - 0 1");
        assert!(e.play_uci_move("a7a8q"));
        assert_eq!(e.pos.board.square[56], crate::types::WHITE_QUEEN as u8);

        let mut e = Box::new(Engine::with_hash_mb(1));
        e.get_board("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
        assert!(e.play_uci_move("e1g1"));
        assert_eq!(e.pos.board.square[6], crate::types::WHITE_KING as u8);

        let mut e = Box::new(Engine::with_hash_mb(1));
        e.get_board("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1");
        assert!(e.play_uci_move("e5d6"));
        assert_eq!(e.pos.board.square[43], crate::types::WHITE_PAWN as u8);
        assert_eq!(e.pos.board.square[35], 0);
    }
}
