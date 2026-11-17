use gull::communication;
use gull::position::Engine;
use gull::search::{NullReporter, SearchLimits};
use gull::types::{move_to_string, uci_score};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("bench") {
        let depth = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(6);
        let fen = args.get(2).map(String::as_str).unwrap_or("startpos");
        bench(depth, fen);
    } else if args.first().map(String::as_str) == Some("eval") {
        if let Some(fen) = args.get(1) {
            let mut e = Engine::new();
            e.set_fen(fen);
            e.evaluate();
            println!("{}", e.score());
        }
    } else {
        communication::run();
    }
}

fn bench(depth: u32, fen: &str) {
    let mut e = Engine::with_hash_mb(8);
    e.reporter = Box::new(NullReporter);
    e.initialize_search(true);
    if fen != "startpos" {
        e.set_fen(fen);
    }
    let start = std::time::Instant::now();
    let r = e.go(&SearchLimits {
        depth,
        infinite: true,
        ..SearchLimits::default()
    });
    let ms = start.elapsed().as_millis().max(1) as u64;
    let (kind, value) = uci_score(r.score);
    println!(
        "info depth {} seldepth {} score {} {} nodes {} nps {} time {}",
        r.depth,
        r.selective_depth,
        kind,
        value,
        r.nodes,
        r.nodes * 1000 / ms,
        ms
    );
    println!("bestmove {}", move_to_string(r.best_move));
}
