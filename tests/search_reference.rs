//! Exactness tests for the search against the C++ reference
//! (`./src/LazyGull bench <depth> <fen>`).

use gull::position::Engine;
use gull::search::{NullReporter, SearchLimits};
use gull::types::{move_to_string, uci_score};

struct Case {
    fen: String,
    depth: u32,
    best: String,
    kind: String,
    value: i32,
    nodes: u64,
}

fn cases() -> Vec<Case> {
    include_str!("fixtures/search.tsv")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            assert_eq!(f.len(), 6, "bad fixture line: {line}");
            Case {
                fen: f[0].to_string(),
                depth: f[1].parse().unwrap(),
                best: f[2].to_string(),
                kind: f[3].to_string(),
                value: f[4].parse().unwrap(),
                nodes: f[5].parse().unwrap(),
            }
        })
        .collect()
}

#[test]
fn search_matches_cpp_reference() {
    let cases = cases();
    assert!(cases.len() >= 40, "expected a substantial fixture set");
    for c in &cases {
        let mut e = Engine::with_hash_mb(8);
        e.reporter = Box::new(NullReporter);
        e.init_search(true);
        e.get_board(&c.fen);
        let r = e.go(&SearchLimits {
            depth: c.depth,
            infinite: true,
            ..SearchLimits::default()
        });
        let (kind, value) = uci_score(r.score);
        assert_eq!(
            move_to_string(r.best_move),
            c.best,
            "best move at depth {} for {}",
            c.depth,
            c.fen
        );
        assert_eq!(
            kind, c.kind,
            "score kind at depth {} for {}",
            c.depth, c.fen
        );
        assert_eq!(value, c.value, "score at depth {} for {}", c.depth, c.fen);
        assert_eq!(r.nodes, c.nodes, "nodes at depth {} for {}", c.depth, c.fen);
    }
}
