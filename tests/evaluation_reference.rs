//! Exactness tests for the evaluation function against captured reference
//! values.

use gull::position::Engine;
use gull::tables::PAWN_HASH_MASK;

struct Case {
    fen: String,
    score: i32,
    pst: i32,
    material: i32,
    pawn_score: i32,
    shelter: [i32; 2],
    passer: [i32; 2],
    draw: [i32; 2],
    pawn_key: u64,
    key: u64,
}

fn cases() -> Vec<Case> {
    include_str!("fixtures/eval.tsv")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            assert_eq!(f.len(), 13, "bad fixture line: {line}");
            Case {
                fen: f[0].to_string(),
                score: f[1].parse().unwrap(),
                pst: f[2].parse().unwrap(),
                material: f[3].parse().unwrap(),
                pawn_score: f[4].parse().unwrap(),
                shelter: [f[5].parse().unwrap(), f[6].parse().unwrap()],
                passer: [f[7].parse().unwrap(), f[8].parse().unwrap()],
                draw: [f[9].parse().unwrap(), f[10].parse().unwrap()],
                pawn_key: u64::from_str_radix(f[11], 16).unwrap(),
                key: u64::from_str_radix(f[12], 16).unwrap(),
            }
        })
        .collect()
}

#[test]
fn evaluation_matches_cpp_reference() {
    let cases = cases();
    assert!(cases.len() >= 40, "expected a substantial fixture set");
    for c in &cases {
        let mut e = Engine::with_hash_mb(1);
        e.get_board(&c.fen);
        e.evaluate();
        let st = e.pos.cur();
        assert_eq!(st.key, c.key, "zobrist key: {}", c.fen);
        assert_eq!(st.pawn_key, c.pawn_key, "pawn key: {}", c.fen);
        assert_eq!(st.score as i32, c.score, "score: {}", c.fen);
        assert_eq!(st.pst, c.pst, "pst: {}", c.fen);
        assert_eq!(st.material, c.material, "material: {}", c.fen);
        let pe = &e.pawn_hash[(st.pawn_key as usize) & PAWN_HASH_MASK];
        assert_eq!(pe.score, c.pawn_score, "pawn score: {}", c.fen);
        assert_eq!(pe.shelter[0] as i32, c.shelter[0], "shelter w: {}", c.fen);
        assert_eq!(pe.shelter[1] as i32, c.shelter[1], "shelter b: {}", c.fen);
        assert_eq!(pe.passer[0] as i32, c.passer[0], "passer w: {}", c.fen);
        assert_eq!(pe.passer[1] as i32, c.passer[1], "passer b: {}", c.fen);
        assert_eq!(pe.draw[0] as i32, c.draw[0], "draw w: {}", c.fen);
        assert_eq!(pe.draw[1] as i32, c.draw[1], "draw b: {}", c.fen);
    }
}
