//! Pure-Rust Syzygy tablebase probing.
//!
//! The probing itself is provided by the MIT-licensed `pyrrhic-rs` crate, which
//! is a Rust transliteration of the Fathom/Pyrrhic probe code.  This module
//! supplies Gull's attack tables through the `EngineAdapter` interface and
//! converts tablebase results into Gull moves and scores.

use std::sync::Arc;

use pyrrhic_rs::{
    Color as TbColor, DtzProbeValue, EngineAdapter, Piece as TbPiece, TableBases, WdlProbeResult,
};

use crate::tables::tables;
use crate::types::*;

/// Adapter that lets the tablebase code use Gull's precomputed attack tables.
#[derive(Clone, Copy, Debug, Default)]
pub struct GullAdapter;

impl EngineAdapter for GullAdapter {
    #[inline]
    fn pawn_attacks(color: TbColor, square: u64) -> u64 {
        let c = match color {
            TbColor::White => WHITE,
            TbColor::Black => BLACK,
        };
        tables().pawn_attacks[c][square as usize]
    }

    #[inline]
    fn knight_attacks(square: u64) -> u64 {
        tables().knight_attacks[square as usize]
    }

    #[inline]
    fn bishop_attacks(square: u64, occupied: u64) -> u64 {
        tables().bishop_attacks(square as usize, occupied)
    }

    #[inline]
    fn rook_attacks(square: u64, occupied: u64) -> u64 {
        tables().rook_attacks(square as usize, occupied)
    }

    #[inline]
    fn queen_attacks(square: u64, occupied: u64) -> u64 {
        tables().queen_attacks(square as usize, occupied)
    }

    #[inline]
    fn king_attacks(square: u64) -> u64 {
        tables().king_attack_area[square as usize]
    }
}

pub type GullTableBases = TableBases<GullAdapter>;

/// Load a colon-separated Syzygy path.  An empty path disables probing.
pub fn load(path: &str) -> Result<Option<(Arc<GullTableBases>, usize)>, String> {
    let path = path.trim();
    if path.is_empty() || path == "<empty>" {
        return Ok(None);
    }
    let max_pieces =
        path_max_pieces(path).ok_or_else(|| "no Syzygy .rtbw/.rtbz files found".to_string())?;
    GullTableBases::new(path)
        .map(|tb| Some((Arc::new(tb), max_pieces)))
        .map_err(|e| format!("{e:?}"))
}

fn path_max_pieces(path: &str) -> Option<usize> {
    let mut max_pieces = 0;
    let separator = if cfg!(windows) { ';' } else { ':' };
    for part in path.split(separator) {
        let p = std::path::Path::new(part);
        let mut consider = |file: &std::path::Path| {
            if !matches!(
                file.extension().and_then(|x| x.to_str()),
                Some("rtbw" | "rtbz")
            ) {
                return;
            }
            let Some(stem) = file.file_stem().and_then(|x| x.to_str()) else {
                return;
            };
            let pieces = stem
                .chars()
                .filter(|c| c.is_ascii_alphabetic() && *c != 'v')
                .count();
            max_pieces = max_pieces.max(pieces);
        };
        if p.is_file() {
            consider(p);
        } else if let Ok(entries) = std::fs::read_dir(p) {
            for entry in entries.flatten() {
                consider(&entry.path());
            }
        }
    }
    (max_pieces > 0).then_some(max_pieces)
}

#[inline]
pub fn wdl_score(wdl: WdlProbeResult) -> i32 {
    match wdl {
        WdlProbeResult::Loss => -TB_MATE_VALUE,
        WdlProbeResult::BlessedLoss => -TB_CURSED_MATE_VALUE,
        WdlProbeResult::Draw => 0,
        WdlProbeResult::CursedWin => TB_CURSED_MATE_VALUE,
        WdlProbeResult::Win => TB_MATE_VALUE,
    }
}

/// Convert a root DTZ result into a Gull move.  `to` selects the bishop
/// promotion colour when needed.
pub fn root_move(dtz: &pyrrhic_rs::DtzResult) -> Option<i32> {
    if dtz.from_square >= 64 || dtz.to_square >= 64 {
        return None;
    }
    let from = dtz.from_square as i32;
    let to = dtz.to_square as i32;
    let mut flags = 0i32;
    if dtz.ep {
        flags |= FLAG_EN_PASSANT;
    } else {
        flags |= match dtz.promotion {
            TbPiece::Pawn => 0,
            TbPiece::Knight => FLAG_PROMOTION_KNIGHT,
            TbPiece::Bishop => {
                if bit(to as usize) & LIGHT_AREA != 0 {
                    FLAG_PROMOTION_LIGHT_BISHOP
                } else {
                    FLAG_PROMOTION_DARK_BISHOP
                }
            }
            TbPiece::Rook => FLAG_PROMOTION_ROOK,
            TbPiece::Queen => FLAG_PROMOTION_QUEEN,
            TbPiece::King => return None,
        };
    }
    Some((from << 6) | to | flags)
}

pub fn root_dtz(value: DtzProbeValue) -> Option<pyrrhic_rs::DtzResult> {
    match value {
        DtzProbeValue::DtzResult(dtz) => Some(dtz),
        DtzProbeValue::Stalemate | DtzProbeValue::Checkmate | DtzProbeValue::Failed => None,
    }
}
