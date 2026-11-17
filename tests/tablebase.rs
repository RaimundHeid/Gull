use std::sync::{Arc, Mutex};

use gull::position::Engine;
use gull::tablebase::{self, GullAdapter};
use gull::types::*;
use pyrrhic_rs::EngineAdapter;

static TB_LOCK: Mutex<()> = Mutex::new(());

const KQK: &str = "8/8/8/8/8/8/Q7/K1k5 w - - 0 1";

fn fixture_path() -> String {
    format!("{}/tests/fixtures/syzygy", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn adapter_uses_gull_attack_tables() {
    let t = gull::tables::tables();
    for square_index in 0..64u64 {
        assert_eq!(
            GullAdapter::pawn_attacks(pyrrhic_rs::Color::White, square_index),
            t.pawn_attacks[WHITE][square_index as usize]
        );
        assert_eq!(
            GullAdapter::pawn_attacks(pyrrhic_rs::Color::Black, square_index),
            t.pawn_attacks[BLACK][square_index as usize]
        );
        assert_eq!(
            GullAdapter::knight_attacks(square_index),
            t.knight_attacks[square_index as usize]
        );
        assert_eq!(
            GullAdapter::king_attacks(square_index),
            t.king_attack_area[square_index as usize]
        );
        assert_eq!(
            GullAdapter::bishop_attacks(square_index, 0x00FF_0000_FF00_0000),
            t.bishop_attacks(square_index as usize, 0x00FF_0000_FF00_0000)
        );
        assert_eq!(
            GullAdapter::rook_attacks(square_index, 0x00FF_0000_FF00_0000),
            t.rook_attacks(square_index as usize, 0x00FF_0000_FF00_0000)
        );
    }
}

#[test]
fn loads_kqvk_and_probes_root() {
    let _lock = TB_LOCK.lock().unwrap();
    let mut e = Box::new(Engine::with_hash_mb(1));
    assert_eq!(e.set_syzygy_path(&fixture_path()).unwrap(), 3);
    e.set_fen(KQK);
    let r = e.go(&gull::search::SearchLimits {
        depth: 5,
        ..Default::default()
    });
    assert_ne!(r.best_move, 0);
    assert_eq!(r.nodes, 1, "root TB result should bypass search");
    assert!(r.score > 20000, "expected a Syzygy win, got {}", r.score);
    assert_eq!(r.principal_variation, vec![r.best_move]);
}

#[test]
fn empty_path_disables_tablebases() {
    let _lock = TB_LOCK.lock().unwrap();
    let mut e = Box::new(Engine::with_hash_mb(1));
    e.set_syzygy_path(&fixture_path()).unwrap();
    assert_eq!(e.tablebase_pieces(), 3);
    e.set_syzygy_path("<empty>").unwrap();
    assert_eq!(e.tablebase_pieces(), 0);
}

#[test]
fn worker_forks_share_tablebase_handle() {
    let _lock = TB_LOCK.lock().unwrap();
    let mut e = Engine::with_hash_mb(1);
    e.set_syzygy_path(&fixture_path()).unwrap();
    let w = e.fork_for_thread(1, 1 << 10);
    assert!(Arc::ptr_eq(
        e.tablebases.as_ref().unwrap(),
        w.tablebases.as_ref().unwrap()
    ));
}

#[test]
fn invalid_path_reports_error() {
    let _lock = TB_LOCK.lock().unwrap();
    let mut e = Engine::with_hash_mb(1);
    assert!(e.set_syzygy_path("/definitely/not/a/syzygy/path").is_err());
    assert_eq!(e.tablebase_pieces(), 0);
}

#[test]
fn tb_scores_match_reference_values() {
    use pyrrhic_rs::WdlProbeResult::*;
    assert_eq!(tablebase::wdl_score(Loss), -TB_MATE_VALUE);
    assert_eq!(tablebase::wdl_score(BlessedLoss), -TB_CURSED_MATE_VALUE);
    assert_eq!(tablebase::wdl_score(Draw), 0);
    assert_eq!(tablebase::wdl_score(CursedWin), TB_CURSED_MATE_VALUE);
    assert_eq!(tablebase::wdl_score(Win), TB_MATE_VALUE);
}
