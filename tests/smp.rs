use std::sync::Arc;
use std::thread;
use std::time::Duration;

use gull::position::{Engine, TranspositionEntry};
use gull::search::SearchLimits;
use gull::types::{move_to_string, Color};

fn assert_legal_after_do(e: &mut Engine, chess_move: i32) {
    let color = e.pos.current().side_to_move as Color;
    e.make_move(color, chess_move);
    let king = e.king(color);
    let safe = !e.is_square_attacked(
        gull::types::least_significant_bit(king),
        gull::types::opponent(color),
    );
    e.unmake_move(color, chess_move);
    assert!(safe, "illegal best move {}", move_to_string(chess_move));
}

#[test]
fn lazy_smp_search_is_legal_and_shares_tables() {
    let mut e = Engine::with_hash_mb(8);
    e.settings.num_threads = 4;
    e.initialize_search(true);
    e.set_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let worker = e.fork_for_thread(1, 1 << 12);
    assert!(Arc::ptr_eq(
        &e.transposition_table,
        &worker.transposition_table
    ));
    assert!(Arc::ptr_eq(
        &e.principal_variation_table,
        &worker.principal_variation_table
    ));

    let r = e.go(&SearchLimits {
        depth: 6,
        ..SearchLimits::default()
    });
    assert!(r.nodes > 0);
    assert_ne!(r.best_move, 0);
    assert_legal_after_do(&mut e, r.best_move);
}

#[test]
fn lazy_smp_stop_joins_workers() {
    let mut e = Engine::with_hash_mb(8);
    e.settings.num_threads = 4;
    e.initialize_search(true);
    e.set_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let control = e.control.clone();
    let handle = thread::spawn(move || {
        e.go(&SearchLimits {
            depth: 256,
            infinite: true,
            ..SearchLimits::default()
        })
    });
    thread::sleep(Duration::from_millis(100));
    control.request_stop();
    let r = handle.join().expect("search thread");
    assert!(r.depth >= 0);
    assert_ne!(r.best_move, 0);
}

#[test]
fn shared_tt_is_visible_across_threads() {
    let e = Engine::with_hash_mb(1);
    let entry = TranspositionEntry {
        key: 0xDEAD_BEEF,
        date: 3,
        chess_move: 0x1234,
        low: -7,
        high: 11,
        flags: 0x55,
        low_depth: 9,
        high_depth: 10,
    };
    e.transposition_table.store(17, entry);
    let worker = e.fork_for_thread(3, 1 << 10);
    assert_eq!(worker.transposition_table.load(17), entry);
    worker.transposition_table.store(18, entry);
    assert_eq!(e.transposition_table.load(18), entry);
}

#[test]
fn max_thread_setting_is_clamped() {
    let mut e = Engine::with_hash_mb(1);
    e.set_threads(256);
    assert_eq!(e.settings.num_threads, 256);
    e.set_threads(0);
    assert_eq!(e.settings.num_threads, 1);
    e.set_threads(999);
    assert_eq!(e.settings.num_threads, 256);
    e.settings.num_threads = 256;
    // Forking 256 workers must remain safe even though only a subset will be
    // used by a short search.
    let workers: Vec<Engine> = (1..256).map(|id| e.fork_for_thread(id, 1 << 10)).collect();
    assert_eq!(workers.len(), 255);
    assert!(workers
        .iter()
        .all(|w| Arc::ptr_eq(&w.transposition_table, &e.transposition_table)));
}

#[test]
fn concurrent_tt_stores_and_loads_are_safe() {
    let e = Engine::with_hash_mb(4);
    let mut handles = Vec::new();
    for thread_index in 0..8 {
        let hash = Arc::clone(&e.transposition_table);
        handles.push(thread::spawn(move || {
            for i in 0..20_000usize {
                let idx = (thread_index * 31_337 + i) & 0x3FFF;
                let entry = TranspositionEntry {
                    key: (thread_index as u32) << 24 | i as u32,
                    date: 7,
                    chess_move: 1,
                    low: -(i as i16),
                    high: i as i16,
                    flags: 0,
                    low_depth: 1,
                    high_depth: 2,
                };
                hash.store(idx, entry);
                let got = hash.load(idx);
                // Concurrent writers may tear the two 64-bit words, but each
                // access itself must be safe and produce a readable entry.
                let _ = got.key;
                let _ = got.high_depth;
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn node_counter_is_shared_across_workers() {
    let e = Engine::with_hash_mb(1);
    e.shared_nodes
        .store(0, std::sync::atomic::Ordering::Relaxed);
    let w = e.fork_for_thread(2, 1 << 10);
    w.shared_nodes
        .fetch_add(1234, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        e.shared_nodes.load(std::sync::atomic::Ordering::Relaxed),
        1234
    );
    assert!(e.total_nodes() >= 1234);
}
