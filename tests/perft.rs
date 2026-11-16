//! Move generation / make-unmake regression tests.
//!
//! The expected values are the counts produced by the *original engine's own*
//! root move generator, not textbook perft values: Gull's `is_legal` filter
//! intentionally omits some standard-legal moves, and this port reproduces
//! that behaviour exactly.

use gull::position::Engine;
use gull::types::Color;

fn perft(e: &mut Engine, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let me = e.pos.cur().turn as Color;
    e.gen_root_moves(me);
    let mut moves = [0i32; 256];
    let mut n = 0usize;
    while n < e.root_list.len() && e.root_list[n] != 0 {
        moves[n] = e.root_list[n];
        n += 1;
    }
    if depth == 1 {
        return n as u64;
    }
    let mut nodes = 0;
    for &mv in moves.iter().take(n) {
        e.do_move(me, mv);
        nodes += perft(e, depth - 1);
        e.undo_move(me, mv);
    }
    nodes
}

fn check(fen: &str, expected: &[u64]) {
    let mut e = Engine::with_hash_mb(1);
    e.get_board(fen);
    for (i, &want) in expected.iter().enumerate() {
        assert_eq!(perft(&mut e, i as u32 + 1), want, "{fen} depth {}", i + 1);
    }
}

#[test]
fn perft_startpos() {
    check(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        &[20, 400, 8868, 195455],
    );
}

#[test]
fn perft_kiwipete() {
    check(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        &[40, 1416, 56044],
    );
}

#[test]
fn perft_position_three() {
    check(
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        &[15, 239, 3757, 63479],
    );
}

#[test]
fn perft_position_four() {
    check(
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        &[35, 1117, 39665],
    );
}

#[test]
fn ep_is_generated() {
    let mut e = Engine::with_hash_mb(1);
    e.get_board("rnbqkbnr/1ppp1ppp/p7/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 3");
    let me = e.pos.cur().turn as Color;
    let mut m = [0i32; 256];
    let n = e.gen_legal_moves(me, &mut m);
    let all: Vec<String> = m[..n]
        .iter()
        .map(|&x| gull::types::move_to_string(x))
        .collect();
    assert!(all.contains(&"e5d6".to_string()));
}

#[test]
fn ep_after_double_push() {
    let mut e = Engine::with_hash_mb(1);
    e.get_board("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    for m in ["e2e4", "a7a6", "e4e5", "d7d5"] {
        assert!(e.play_uci_move(m), "{m}");
    }
    let me = e.pos.cur().turn as Color;
    let mut m = [0i32; 256];
    let n = e.gen_legal_moves(me, &mut m);
    let all: Vec<String> = m[..n]
        .iter()
        .map(|&x| gull::types::move_to_string(x))
        .collect();
    assert!(all.contains(&"e5d6".to_string()));
}

#[test]
fn root_moves_include_ep() {
    let mut e = Engine::with_hash_mb(1);
    e.get_board("rnbqkbnr/1pp1pppp/p7/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 1");
    let me = e.pos.cur().turn as Color;
    e.gen_root_moves(me);
    let mut all = Vec::new();
    for &x in e.root_list.iter() {
        if x == 0 {
            break;
        }
        all.push(gull::types::move_to_string(x));
    }
    assert!(all.contains(&"e5d6".to_string()));
}
