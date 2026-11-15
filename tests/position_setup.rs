use gull::position::Engine;
use gull::search::SearchLimits;

#[test]
fn uci_moves_then_search() {
    let mut e = Engine::with_hash_mb(8);
    e.get_board("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    assert!(e.play_uci_move("e2e4"));
    assert!(e.play_uci_move("e7e5"));
    assert_eq!(e.pos.cur, 0);
    let r = e.go(&SearchLimits {
        depth: 2,
        infinite: true,
        ..SearchLimits::default()
    });
    assert!(r.depth >= 1);
    assert_ne!(r.best_move, 0);
}

#[test]
fn position_reset_keeps_keys_consistent() {
    let mut e = Engine::with_hash_mb(1);
    e.get_board("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    e.play_uci_move("g1f3");
    assert_eq!(e.pos.cur, 0);
    let fen = e.current_fen();
    let mut d = Engine::with_hash_mb(1);
    d.get_board(&fen);
    assert_eq!(e.pos.cur().key, d.pos.cur().key);
}

#[test]
fn applies_a_move_that_the_legality_helper_misses() {
    // In this position `gen_legal_moves` (the old UCI validation path) omits
    // the perfectly legal ...Nxe4.  The UCI layer must apply the GUI move
    // directly, as the C++ front end does, or the engine desynchronises.
    let mut e = Engine::with_hash_mb(1);
    e.get_board("rnbqkb1r/pppp1ppp/4pn2/8/4P3/5N1P/PPPP1PP1/RNBQKB1R b KQkq - 0 3");
    assert!(e.play_uci_move("f6e4"));
    assert_eq!(e.pos.cur, 0);
    assert_eq!(e.pos.board.square[28] as usize, gull::types::BLACK_KNIGHT);
    assert_eq!(e.pos.cur().turn as usize, gull::types::WHITE);
    let fen = e.current_fen();
    assert!(fen.starts_with("rnbqkb1r/pppp1ppp/4p3/8/4n3/5N1P/PPPP1PP1/RNBQKB1R w"));
}
