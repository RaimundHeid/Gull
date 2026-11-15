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
