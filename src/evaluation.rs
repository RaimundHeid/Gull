//! Evaluation: an exact port of the C++ `evaluation()` and its helpers.
//!
//! Scores are stored as packed opening/endgame pairs (see `compose`).  The
//! code deliberately follows the C++ control flow, including the places
//! where `Current->score` (an `int16_t`) is truncated or modified in place.

use crate::position::{Engine, PawnEntry, Position};
use crate::tables::*;
use crate::types::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct EvalTrace {
    pub score: i32,
    pub piece_square_score: i32,
    pub material: i32,
    pub pawn_score: i32,
}

#[derive(Clone, Copy)]
struct EvalInfo {
    score: i32,
    white_king: usize,
    black_king: usize,
    evaluation_multiplier: i32,
    occupancy: u64,
    white_area: u64,
    black_area: u64,
    white_free: u64,
    black_free: u64,
    white_king_attacks: u32,
    black_king_attacks: u32,
    pawn_index: usize,
    material: Material,
}

#[derive(Clone, Copy)]
struct PawnEvalInfo {
    white_king: usize,
    black_king: usize,
    score: i32,
    white_pawn_attacks: u64,
    black_pawn_attacks: u64,
    white_double_pawn_attacks: u64,
    black_double_pawn_attacks: u64,
}

#[inline(always)]
fn free_bitboard(eval_info: &EvalInfo, color: Color) -> u64 {
    if color == WHITE {
        eval_info.white_free
    } else {
        eval_info.black_free
    }
}

#[inline(always)]
fn area_bitboard(eval_info: &EvalInfo, color: Color) -> u64 {
    if color == WHITE {
        eval_info.white_area
    } else {
        eval_info.black_area
    }
}

#[inline(always)]
fn king_square(eval_info: &EvalInfo, color: Color) -> usize {
    if color == WHITE {
        eval_info.white_king
    } else {
        eval_info.black_king
    }
}

#[inline(always)]
fn set_free_area(eval_info: &mut EvalInfo, color: Color, v: u64) {
    if color == WHITE {
        eval_info.white_free = v;
    } else {
        eval_info.black_free = v;
    }
}

#[inline(always)]
fn add_free_area(eval_info: &mut EvalInfo, color: Color, v: u64) {
    if color == WHITE {
        eval_info.white_free |= v;
    } else {
        eval_info.black_free |= v;
    }
}

#[inline(always)]
fn king_attack_weight(eval_info: &EvalInfo, color: Color) -> u32 {
    if color == WHITE {
        eval_info.white_king_attacks
    } else {
        eval_info.black_king_attacks
    }
}

#[inline(always)]
fn add_king_attack(eval_info: &mut EvalInfo, color: Color, x: i32) {
    let v = x as u32;
    if color == WHITE {
        eval_info.white_king_attacks = eval_info.white_king_attacks.wrapping_add(v);
    } else {
        eval_info.black_king_attacks = eval_info.black_king_attacks.wrapping_add(v);
    }
}

#[inline(always)]
fn add_eval(color: Color, var: &mut i32, x: i32) {
    if color == WHITE {
        *var += x;
    } else {
        *var -= x;
    }
}

#[inline(always)]
fn subtract_eval(color: Color, var: &mut i32, x: i32) {
    if color == WHITE {
        *var -= x;
    } else {
        *var += x;
    }
}

#[inline(always)]
fn eval_pair(arr: &[i32], y: usize) -> i32 {
    compose(arr[y * 2], arr[y * 2 + 1])
}

#[inline(always)]
fn free_area(eval_info: &EvalInfo, color: Color) -> u64 {
    free_bitboard(eval_info, color)
}

/// `evaluation()`.
pub fn evaluate(e: &mut Engine) {
    e.evaluate();
}

impl Engine {
    #[inline(always)]
    pub fn evaluate(&mut self) {
        let current = self.pos.current;
        let key = self.pos.states[current].key;
        if self.pos.states[current].eval_key == key {
            return;
        }
        self.pos.states[current].eval_key = key;

        let tables = self.tables;
        let occupancy = self.piece_all();
        let white_king = least_significant_bit(self.king(WHITE));
        let black_king = least_significant_bit(self.king(BLACK));
        let white_pawn_attacks =
            Self::shift_west(WHITE, self.pawn(WHITE)) | Self::shift_east(WHITE, self.pawn(WHITE));
        let black_pawn_attacks =
            Self::shift_west(BLACK, self.pawn(BLACK)) | Self::shift_east(BLACK, self.pawn(BLACK));
        let white_area = (tables.king_attack_area[white_king] | self.king(WHITE))
            & ((!white_pawn_attacks) | black_pawn_attacks);
        let black_area = (tables.king_attack_area[black_king] | self.king(BLACK))
            & ((!black_pawn_attacks) | white_pawn_attacks);
        self.pos.states[current].pawn_attacks[WHITE] = white_pawn_attacks;
        self.pos.states[current].pawn_attacks[BLACK] = black_pawn_attacks;
        self.pos.states[current].attacks[WHITE] = white_pawn_attacks;
        self.pos.states[current].attacks[BLACK] = black_pawn_attacks;
        self.pos.states[current].passed_pawns = 0;
        self.pos.states[current].threats = (white_pawn_attacks & self.non_pawn(BLACK))
            | (black_pawn_attacks & self.non_pawn(WHITE));

        let mut eval_info = EvalInfo {
            score: self.pos.states[current].piece_square_score,
            white_king,
            black_king,
            evaluation_multiplier: 32,
            occupancy,
            white_area,
            black_area,
            white_free: 0,
            black_free: 0,
            white_king_attacks: 0,
            black_king_attacks: 0,
            pawn_index: 0,
            material: Material::default(),
        };

        // White.
        self.pos.states[current].xray_attacks[WHITE] = 0;
        let white_free = self.queen(BLACK)
            | self.king(BLACK)
            | (!(black_pawn_attacks | self.pawn(WHITE) | self.king(WHITE)));
        set_free_area(&mut eval_info, WHITE, white_free);
        let blocked = population_count(Self::shift(BLACK, eval_info.occupancy) & self.pawn(WHITE))
            * eval_pair(&tables.pawn_special, PAWN_BLOCKED);
        subtract_eval(WHITE, &mut eval_info.score, blocked);
        if (white_pawn_attacks & eval_info.black_area) != 0 {
            eval_info.white_king_attacks = king_attack_score() as u32;
        }
        self.eval_queens(WHITE, &mut eval_info);
        add_free_area(&mut eval_info, WHITE, self.rook(BLACK));
        self.eval_rooks(WHITE, &mut eval_info);
        add_free_area(&mut eval_info, WHITE, self.minor(BLACK));
        self.eval_bishops(WHITE, &mut eval_info);
        self.eval_knights(WHITE, &mut eval_info);

        // Black.
        self.pos.states[current].xray_attacks[BLACK] = 0;
        let black_free = self.queen(WHITE)
            | self.king(WHITE)
            | (!(white_pawn_attacks | self.pawn(BLACK) | self.king(BLACK)));
        set_free_area(&mut eval_info, BLACK, black_free);
        let blocked = population_count(Self::shift(WHITE, eval_info.occupancy) & self.pawn(BLACK))
            * eval_pair(&tables.pawn_special, PAWN_BLOCKED);
        subtract_eval(BLACK, &mut eval_info.score, blocked);
        if (black_pawn_attacks & eval_info.white_area) != 0 {
            eval_info.black_king_attacks = king_attack_score() as u32;
        }
        self.eval_queens(BLACK, &mut eval_info);
        add_free_area(&mut eval_info, BLACK, self.rook(WHITE));
        self.eval_rooks(BLACK, &mut eval_info);
        add_free_area(&mut eval_info, BLACK, self.minor(WHITE));
        self.eval_bishops(BLACK, &mut eval_info);
        self.eval_knights(BLACK, &mut eval_info);

        // Pawn hash / pawn structure.
        let pawn_key = self.pos.states[current].pawn_key;
        let idx = (pawn_key & self.pawn_hash_mask as u64) as usize;
        if (pawn_key >> 16) != self.pawn_hash[idx].key48() {
            eval_pawn_structure(&self.pos, &self.tables, &mut self.pawn_hash[idx]);
        }
        eval_info.pawn_index = idx;
        eval_info.score += self.pawn_hash[idx].score;
        let pawn_score = self.pawn_hash[idx].score;

        self.eval_king(WHITE, &mut eval_info);
        self.eval_king(BLACK, &mut eval_info);
        self.pos.states[current].attacks[WHITE] |= tables.king_attack_area[eval_info.white_king];
        self.pos.states[current].attacks[BLACK] |= tables.king_attack_area[eval_info.black_king];

        self.eval_passer(WHITE, &mut eval_info);
        self.eval_pieces(WHITE, &mut eval_info);
        self.eval_passer(BLACK, &mut eval_info);
        self.eval_pieces(BLACK, &mut eval_info);

        if (self.pos.states[current].material & FLAG_UNUSUAL_MATERIAL) != 0 {
            self.eval_unusual_material(&eval_info);
            return;
        }
        let material = tables.material[self.pos.states[current].material as usize];
        eval_info.material = material;
        let phase = material.phase as i32;
        let base = material.score as i32
            + ((opening(eval_info.score) * phase) + (endgame(eval_info.score) * (128 - phase)))
                / 128;
        self.pos.states[current].score = base as i16;

        if self.pos.states[current].ply >= 50 {
            self.pos.states[current].score /= 2;
        }
        let s = self.pos.states[current].score as i32;
        if s > 0 {
            eval_info.evaluation_multiplier = material.evaluation_multiplier[WHITE] as i32;
            if (material.flags & FLAG_CALL_EVAL_ENDGAME_WHITE) != 0 {
                self.eval_endgame(WHITE, &mut eval_info);
            }
            let cs = self.pos.states[current].score as i32;
            let draw = self.pawn_hash[eval_info.pawn_index].draw[WHITE] as i32;
            self.pos.states[current].score = (cs - (cs.min(100) * draw) / 64) as i16;
        } else if s < 0 {
            eval_info.evaluation_multiplier = material.evaluation_multiplier[BLACK] as i32;
            if (material.flags & FLAG_CALL_EVAL_ENDGAME_BLACK) != 0 {
                self.eval_endgame(BLACK, &mut eval_info);
            }
            let cs = self.pos.states[current].score as i32;
            let draw = self.pawn_hash[eval_info.pawn_index].draw[BLACK] as i32;
            self.pos.states[current].score = (cs + ((-cs).min(100) * draw) / 64) as i16;
        } else {
            eval_info.evaluation_multiplier = (material.evaluation_multiplier[WHITE] as i32)
                .min(material.evaluation_multiplier[BLACK] as i32);
        }
        let cs = self.pos.states[current].score as i32;
        self.pos.states[current].score = ((cs * eval_info.evaluation_multiplier) / 32) as i16;

        if self.pos.states[current].side_to_move != 0 {
            self.pos.states[current].score = (-(self.pos.states[current].score as i32)) as i16;
        }
        self.update_delta();

        let _ = pawn_score;
    }

    /// Evaluate and return a trace of the main components (used by tests).
    pub fn evaluate_trace(&mut self) -> EvalTrace {
        self.evaluate();
        let state = &self.pos.states[self.pos.current];
        let idx = (state.pawn_key & self.pawn_hash_mask as u64) as usize;
        let _ = idx;
        EvalTrace {
            score: state.score as i32,
            piece_square_score: state.piece_square_score,
            material: state.material,
            pawn_score: 0,
        }
    }

    /// `UpdateDelta` macro.
    pub fn update_delta(&mut self) {
        let current = self.pos.current;
        let state = self.pos.states[current];
        if state.captured_piece != 0
            || state.chess_move == 0
            || (state.chess_move as i32 & 0xE000) != 0
            || current == 0
        {
            return;
        }
        let chess_move = state.chess_move as i32;
        let piece = state.moving_piece as usize;
        let from = move_from(chess_move);
        let to = move_to(chess_move);
        let idx = (piece << 12) | (from << 6) | to;
        let prev = self.pos.states[current - 1].score as i32;
        let target = -(state.score as i32) - prev;
        if (self.delta[idx] as i32) <= target {
            self.delta[idx] = target as i16;
        } else {
            self.delta[idx] = self.delta[idx].wrapping_sub(1);
        }
    }

    // -- piece evaluation --------------------------------------------------

    #[inline(always)]
    fn eval_queens(&mut self, color: Color, eval_info: &mut EvalInfo) {
        let tables = self.tables;
        let current = self.pos.current;
        let opponent_color = opponent(color);
        let mut u = self.queen(color);
        while u != 0 {
            let square_index = least_significant_bit(u);
            let b = bit(square_index);
            u ^= b;
            let attacks = tables.queen_attacks(square_index, eval_info.occupancy);
            self.pos.states[current].attacks[color] |= attacks;
            if (tables.queen_attack_mask[square_index] & self.king(opponent_color)) != 0 {
                let v = tables.between[king_square(eval_info, opponent_color)][square_index]
                    & eval_info.occupancy;
                if v != 0 {
                    if has_single_bit(v) {
                        self.pos.states[current].xray_attacks[color] |= v;
                        let square = least_significant_bit(v);
                        let piece = self.pos.board.squares[square] as usize;
                        let mut king_attacked = false;
                        if piece == pawn_piece(color) {
                            if self.pos.board.squares[(square as i32 + Self::push(color)) as usize]
                                == 0
                            {
                                add_eval(
                                    color,
                                    &mut eval_info.score,
                                    eval_pair(&tables.pin, SELF_PAWN_PIN),
                                );
                            }
                        } else if (piece & 1) == color {
                            add_eval(
                                color,
                                &mut eval_info.score,
                                eval_pair(&tables.pin, SELF_PIECE_PIN),
                            );
                            king_attacked = true;
                        } else if piece != pawn_piece(opponent_color)
                            && ((((tables.bishop_attack_mask[square_index]
                                & self.bishop(opponent_color))
                                | (tables.rook_attack_mask[square_index]
                                    & self.rook(opponent_color))
                                | self.queen(opponent_color))
                                & v)
                                == 0)
                        {
                            add_eval(
                                color,
                                &mut eval_info.score,
                                eval_pair(&tables.pin, WEAK_PIN),
                            );
                            if (self.pos.states[current].pawn_attacks[opponent_color] & v) == 0 {
                                king_attacked = true;
                            }
                        }
                        if king_attacked
                            && (attacks & area_bitboard(eval_info, opponent_color)) == 0
                        {
                            add_king_attack(eval_info, color, king_attack_score());
                        }
                    } else {
                        if v == (v & self.minor(opponent_color)) {
                            add_eval(
                                color,
                                &mut eval_info.score,
                                eval_pair(&tables.king_ray, Q_KING_RAY),
                            );
                        }
                    }
                }
            }
            if (attacks & area_bitboard(eval_info, opponent_color)) != 0 {
                add_king_attack(eval_info, color, king_queen_attack_score());
                let mut v = attacks & area_bitboard(eval_info, opponent_color);
                while v != 0 {
                    let s = least_significant_bit(v);
                    clear_lowest_bit(&mut v);
                    if (tables.full_line[square_index][s]
                        & attacks
                        & ((self.rook(color) & tables.rook_attack_mask[square_index])
                            | (self.bishop(color) & tables.bishop_attack_mask[square_index])))
                        != 0
                    {
                        add_king_attack(eval_info, color, 1);
                    }
                }
            }
            let free = free_area(eval_info, color);
            let mob = population_count(attacks & free) as usize;
            let mob_score = tables.mobility[PIECE_TYPE[WHITE_QUEEN] - 1][mob];
            add_eval(color, &mut eval_info.score, mob_score);
            if (attacks & free & self.pawn(opponent_color)) != 0 {
                add_eval(
                    color,
                    &mut eval_info.score,
                    eval_pair(&tables.tactical, TACTICAL_MAJOR_PAWN),
                );
            }
            if (attacks & free & self.minor(opponent_color)) != 0 {
                add_eval(
                    color,
                    &mut eval_info.score,
                    eval_pair(&tables.tactical, TACTICAL_MAJOR_MINOR),
                );
            }
            if (attacks & area_bitboard(eval_info, color)) != 0 {
                add_eval(
                    color,
                    &mut eval_info.score,
                    eval_pair(&tables.king_defence, KING_DEF_QUEEN),
                );
            }
        }
    }

    #[inline(always)]
    fn eval_rooks(&mut self, color: Color, eval_info: &mut EvalInfo) {
        let tables = self.tables;
        let current = self.pos.current;
        let opponent_color = opponent(color);
        let mut u = self.rook(color);
        while u != 0 {
            let square_index = least_significant_bit(u);
            let b = bit(square_index);
            u ^= b;
            let attacks = tables.rook_attacks(square_index, eval_info.occupancy);
            self.pos.states[current].attacks[color] |= attacks;
            if (tables.rook_attack_mask[square_index] & self.king(opponent_color)) != 0 {
                let v = tables.between[king_square(eval_info, opponent_color)][square_index]
                    & eval_info.occupancy;
                if v != 0 {
                    if has_single_bit(v) {
                        self.pos.states[current].xray_attacks[color] |= v;
                        let square = least_significant_bit(v);
                        let piece = self.pos.board.squares[square] as usize;
                        let mut king_attacked = false;
                        if piece == pawn_piece(color) {
                            if self.pos.board.squares[(square as i32 + Self::push(color)) as usize]
                                == 0
                            {
                                add_eval(
                                    color,
                                    &mut eval_info.score,
                                    eval_pair(&tables.pin, SELF_PAWN_PIN),
                                );
                            }
                        } else if (piece & 1) == color {
                            add_eval(
                                color,
                                &mut eval_info.score,
                                eval_pair(&tables.pin, SELF_PIECE_PIN),
                            );
                            king_attacked = true;
                        } else if piece != pawn_piece(opponent_color) {
                            if piece < rook_piece(opponent_color) {
                                add_eval(
                                    color,
                                    &mut eval_info.score,
                                    eval_pair(&tables.pin, WEAK_PIN),
                                );
                                if (self.pos.states[current].pawn_attacks[opponent_color] & v) == 0
                                {
                                    king_attacked = true;
                                }
                            } else if piece == queen_piece(opponent_color) {
                                add_eval(
                                    color,
                                    &mut eval_info.score,
                                    eval_pair(&tables.pin, THREAT_PIN),
                                );
                            }
                        }
                        if king_attacked
                            && (attacks & area_bitboard(eval_info, opponent_color)) == 0
                        {
                            add_king_attack(eval_info, color, king_attack_score());
                        }
                    } else if v == (v & (self.minor(opponent_color) | self.queen(opponent_color))) {
                        add_eval(
                            color,
                            &mut eval_info.score,
                            eval_pair(&tables.king_ray, R_KING_RAY),
                        );
                    }
                }
            }
            if (attacks & area_bitboard(eval_info, opponent_color)) != 0 {
                add_king_attack(eval_info, color, king_r_attack());
                let mut v = attacks & area_bitboard(eval_info, opponent_color);
                while v != 0 {
                    let s = least_significant_bit(v);
                    clear_lowest_bit(&mut v);
                    if (tables.full_line[square_index][s] & attacks & self.major(color)) != 0 {
                        add_king_attack(eval_info, color, 1);
                    }
                }
            }
            let free = free_area(eval_info, color);
            let mob = population_count(attacks & free) as usize;
            let mob_score = tables.mobility[PIECE_TYPE[WHITE_ROOK] - 1][mob];
            add_eval(color, &mut eval_info.score, mob_score);
            if (attacks & free & self.pawn(opponent_color)) != 0 {
                add_eval(
                    color,
                    &mut eval_info.score,
                    eval_pair(&tables.tactical, TACTICAL_MAJOR_PAWN),
                );
            }
            if (attacks & free & self.minor(opponent_color)) != 0 {
                add_eval(
                    color,
                    &mut eval_info.score,
                    eval_pair(&tables.tactical, TACTICAL_MAJOR_MINOR),
                );
            }
            if (attacks & area_bitboard(eval_info, color)) != 0 {
                add_eval(
                    color,
                    &mut eval_info.score,
                    eval_pair(&tables.king_defence, KING_DEF_ROOK),
                );
            }
            self.pos.states[current].threats |= attacks & self.queen(opponent_color);
            if (tables.pawn_paths[color][square_index] & self.pawn(color)) == 0 {
                add_eval(
                    color,
                    &mut eval_info.score,
                    eval_pair(&tables.rook_special, ROOK_HOF),
                );
                let mut hof_score = 0i32;
                if (tables.pawn_paths[color][square_index] & self.pawn(opponent_color)) == 0 {
                    add_eval(
                        color,
                        &mut eval_info.score,
                        eval_pair(&tables.rook_special, ROOK_OF),
                    );
                    if (attacks & self.line(color, 7)) != 0 {
                        hof_score += eval_pair(&tables.rook_special, ROOK_OF_OPEN);
                    } else {
                        let target = attacks
                            & tables.pawn_paths[color][square_index]
                            & self.minor(opponent_color);
                        if target != 0 {
                            if (self.pos.states[current].pawn_attacks[opponent_color] & target) == 0
                            {
                                hof_score += eval_pair(&tables.rook_special, ROOK_OF_MINOR_HAGING);
                                if (tables.pawn_paths[color][square_index]
                                    & self.king(opponent_color))
                                    != 0
                                {
                                    hof_score += eval_pair(&tables.rook_special, ROOK_OF_KING_ATT);
                                }
                            } else {
                                hof_score += eval_pair(&tables.rook_special, ROOK_OF_MINOR_FIXED);
                            }
                        }
                    }
                } else if (attacks
                    & tables.pawn_paths[color][square_index]
                    & self.pawn(opponent_color))
                    != 0
                {
                    let square = least_significant_bit(
                        attacks
                            & tables.pawn_paths[color][square_index]
                            & self.pawn(opponent_color),
                    );
                    if (tables.pawn_support[opponent_color][square] & self.pawn(opponent_color))
                        == 0
                    {
                        hof_score += eval_pair(&tables.rook_special, ROOK_HOF_WEAK_P_ATT);
                    }
                }
                add_eval(color, &mut eval_info.score, hof_score);
                if (tables.pawn_paths[opponent_color][square_index] & attacks & self.major(color))
                    != 0
                {
                    add_eval(color, &mut eval_info.score, hof_score);
                }
            }
            if (b & self.line(color, 6)) != 0
                && ((self.king(opponent_color) | self.pawn(opponent_color))
                    & (self.line(color, 6) | self.line(color, 7)))
                    != 0
            {
                add_eval(
                    color,
                    &mut eval_info.score,
                    eval_pair(&tables.rook_special, ROOK_7TH),
                );
                if (self.king(opponent_color) & self.line(color, 7)) != 0 {
                    add_eval(
                        color,
                        &mut eval_info.score,
                        eval_pair(&tables.rook_special, ROOK_7TH_K8TH),
                    );
                }
                if (self.major(color) & attacks & self.line(color, 6)) != 0 {
                    add_eval(
                        color,
                        &mut eval_info.score,
                        eval_pair(&tables.rook_special, ROOK_7TH_DOUBLED),
                    );
                }
            }
        }
    }

    #[inline(always)]
    fn eval_bishops(&mut self, color: Color, eval_info: &mut EvalInfo) {
        let tables = self.tables;
        let current = self.pos.current;
        let opponent_color = opponent(color);
        let mut u = self.bishop(color);
        while u != 0 {
            let square_index = least_significant_bit(u);
            let b = bit(square_index);
            u ^= b;
            let attacks = tables.bishop_attacks(square_index, eval_info.occupancy);
            self.pos.states[current].attacks[color] |= attacks;
            if (tables.bishop_attack_mask[square_index] & self.king(opponent_color)) != 0 {
                let v = tables.between[king_square(eval_info, opponent_color)][square_index]
                    & eval_info.occupancy;
                if v != 0 {
                    if has_single_bit(v) {
                        self.pos.states[current].xray_attacks[color] |= v;
                        let square = least_significant_bit(v);
                        let piece = self.pos.board.squares[square] as usize;
                        let mut king_attacked = false;
                        if piece == pawn_piece(color) {
                            if self.pos.board.squares[(square as i32 + Self::push(color)) as usize]
                                == 0
                            {
                                add_eval(
                                    color,
                                    &mut eval_info.score,
                                    eval_pair(&tables.pin, SELF_PAWN_PIN),
                                );
                            }
                        } else if (piece & 1) == color {
                            add_eval(
                                color,
                                &mut eval_info.score,
                                eval_pair(&tables.pin, SELF_PIECE_PIN),
                            );
                            king_attacked = true;
                        } else if piece != pawn_piece(opponent_color) {
                            if piece < light_bishop_piece(opponent_color) {
                                add_eval(
                                    color,
                                    &mut eval_info.score,
                                    eval_pair(&tables.pin, STRONG_PIN),
                                );
                                if (self.pos.states[current].pawn_attacks[opponent_color] & v) == 0
                                {
                                    king_attacked = true;
                                }
                            } else if piece >= rook_piece(opponent_color) {
                                add_eval(
                                    color,
                                    &mut eval_info.score,
                                    eval_pair(&tables.pin, THREAT_PIN),
                                );
                            }
                        }
                        if king_attacked
                            && (attacks & area_bitboard(eval_info, opponent_color)) == 0
                        {
                            add_king_attack(eval_info, color, king_attack_score());
                        }
                    } else if v == (v & (self.knight(opponent_color) | self.major(opponent_color)))
                    {
                        add_eval(
                            color,
                            &mut eval_info.score,
                            eval_pair(&tables.king_ray, B_KING_RAY),
                        );
                    }
                }
            }
            if (attacks & area_bitboard(eval_info, opponent_color)) != 0 {
                add_king_attack(eval_info, color, king_b_attack());
            }
            let free = free_area(eval_info, color);
            let mob = population_count(attacks & free) as usize;
            let mob_score = tables.mobility[PIECE_TYPE[WHITE_LIGHT_BISHOP] - 1][mob];
            add_eval(color, &mut eval_info.score, mob_score);
            if (attacks & free & self.pawn(opponent_color)) != 0 {
                add_eval(
                    color,
                    &mut eval_info.score,
                    eval_pair(&tables.tactical, TACTICAL_MINOR_PAWN),
                );
            }
            if (attacks & free & self.knight(opponent_color)) != 0 {
                add_eval(
                    color,
                    &mut eval_info.score,
                    eval_pair(&tables.tactical, TACTICAL_MINOR_MINOR),
                );
            }
            if (attacks & area_bitboard(eval_info, color)) != 0 {
                add_eval(
                    color,
                    &mut eval_info.score,
                    eval_pair(&tables.king_defence, KING_DEF_BISHOP),
                );
            }
            self.pos.states[current].threats |= attacks & self.major(opponent_color);
            let (light_area, other_area) = if (b & LIGHT_AREA) != 0 {
                (LIGHT_AREA, DARK_AREA)
            } else {
                (DARK_AREA, LIGHT_AREA)
            };
            let _ = other_area;
            let mut v = ((!tables.bishop_forward_rays[color][square_index])
                | (attacks & tables.forward_ranks[color][rank(square_index)]))
                & self.pawn(opponent_color)
                & (!self.pos.states[current].pawn_attacks[opponent_color])
                & light_area;
            while v != 0 {
                let square = least_significant_bit(v);
                clear_lowest_bit(&mut v);
                if ((tables.pawn_support[opponent_color][square]
                    | tables.pawn_paths[opponent_color][square])
                    & self.pawn(opponent_color))
                    == 0
                {
                    add_eval(
                        color,
                        &mut eval_info.score,
                        eval_pair(&tables.bishop_special, BISHOP_NON_FORWARD_PAWN),
                    );
                }
            }
            let mut v =
                tables.bishop_forward_rays[color][square_index] & self.pawn(color) & light_area;
            v |= (v
                & (tables.file[2]
                    | tables.file[3]
                    | tables.file[4]
                    | tables.file[5]
                    | tables.bishop_attack_mask[square_index]))
                >> 8;
            subtract_eval(
                color,
                &mut eval_info.score,
                eval_pair(&tables.bishop_special, BISHOP_PAWN_BLOCK) * population_count(v),
            );
        }
    }

    #[inline(always)]
    fn eval_knights(&mut self, color: Color, eval_info: &mut EvalInfo) {
        let tables = self.tables;
        let current = self.pos.current;
        let opponent_color = opponent(color);
        let mut u = self.knight(color);
        while u != 0 {
            let square_index = least_significant_bit(u);
            let b = bit(square_index);
            u ^= b;
            let attacks = tables.knight_attacks[square_index];
            self.pos.states[current].attacks[color] |= attacks;
            if (attacks & area_bitboard(eval_info, opponent_color)) != 0 {
                add_king_attack(eval_info, color, king_knight_attack_score());
            }
            let free = free_area(eval_info, color);
            let mob = population_count(attacks & free) as usize;
            let mob_score = tables.mobility[PIECE_TYPE[WHITE_KNIGHT] - 1][mob];
            add_eval(color, &mut eval_info.score, mob_score);
            if (attacks & free & self.pawn(opponent_color)) != 0 {
                add_eval(
                    color,
                    &mut eval_info.score,
                    eval_pair(&tables.tactical, TACTICAL_MINOR_PAWN),
                );
            }
            if (attacks & free & self.bishop(opponent_color)) != 0 {
                add_eval(
                    color,
                    &mut eval_info.score,
                    eval_pair(&tables.tactical, TACTICAL_MINOR_MINOR),
                );
            }
            if (attacks & area_bitboard(eval_info, color)) != 0 {
                add_eval(
                    color,
                    &mut eval_info.score,
                    eval_pair(&tables.king_defence, KING_DEF_KNIGHT),
                );
            }
            self.pos.states[current].threats |= attacks & self.major(opponent_color);
            if (b & OUTPOST[color]) != 0
                && (self.pawn(opponent_color)
                    & tables.isolated_pawns[file(square_index)]
                    & tables.forward_ranks[color][rank(square_index)])
                    == 0
            {
                add_eval(
                    color,
                    &mut eval_info.score,
                    eval_pair(&tables.knight_special, KNIGHT_OUTPOST),
                );
                if (self.pos.states[current].pawn_attacks[color] & b) != 0 {
                    add_eval(
                        color,
                        &mut eval_info.score,
                        eval_pair(&tables.knight_special, KNIGHT_OUTPOST_PROTECTED),
                    );
                    if (attacks & free_area(eval_info, color) & self.pawn(opponent_color)) != 0 {
                        add_eval(
                            color,
                            &mut eval_info.score,
                            eval_pair(&tables.knight_special, KNIGHT_OUTPOST_PAWN_ATT),
                        );
                    }
                    if (attacks & free & self.bishop(opponent_color)) != 0 {
                        add_eval(
                            color,
                            &mut eval_info.score,
                            eval_pair(&tables.knight_special, KNIGHT_OUTPOST_BISHOP_ATT),
                        );
                    }
                }
            }
        }
    }

    #[inline(always)]
    fn eval_king(&mut self, color: Color, eval_info: &mut EvalInfo) {
        let tables = self.tables;
        let opponent_color = opponent(color);
        let current = self.pos.current;
        let cnt = 15.min(opening(king_attack_weight(eval_info, color) as i32));
        let mut score = endgame(king_attack_weight(eval_info, color) as i32);
        let shelter_opp = self.pawn_hash[eval_info.pawn_index].shelter[opponent_color] as i32;
        if cnt >= 2 && self.queen(color) != 0 {
            score += (shelter_opp * king_shelter_quad()) / 64;
            let u = self.pos.states[current].attacks[color]
                & area_bitboard(eval_info, opponent_color)
                & (!self.pos.states[current].attacks[opponent_color]);
            if u != 0 {
                score += population_count(u) * king_attack_square();
            }
            if (tables.king_attack_area[king_square(eval_info, opponent_color)]
                & (!(self.piece(opponent_color) | self.pos.states[current].attacks[color])))
                == 0
            {
                score += king_no_moves();
            }
        }
        let mut adjusted = ((score * tables.king_attack_scale[cnt as usize]) >> 3) + shelter_opp;
        if self.queen(color) == 0 {
            adjusted /= 2;
        }
        add_eval(color, &mut eval_info.score, adjusted);
    }

    #[inline(always)]
    fn eval_passer(&mut self, color: Color, eval_info: &mut EvalInfo) {
        let tables = self.tables;
        let current = self.pos.current;
        let opponent_color = opponent(color);
        let mut u = self.pawn_hash[eval_info.pawn_index].passed_pawns[color] as u64;
        while u != 0 {
            let f = least_significant_bit(u);
            clear_lowest_bit(&mut u);
            let passed_pawns = tables.file[f] & self.pawn(color);
            if passed_pawns == 0 {
                continue;
            }
            let square_index = next_bit(opponent_color, passed_pawns);
            let r = relative_rank(color, square_index);
            self.pos.states[current].passed_pawns |= bit(square_index);
            if r <= 2 {
                continue;
            }
            if self.pos.board.squares[(square_index as i32 + Self::push(color)) as usize] == 0 {
                add_eval(color, &mut eval_info.score, tables.passed_pawn_blocked[r]);
            }
            let way = tables.pawn_paths[color][square_index];
            let mut connected = 0;
            let mut supported = false;
            let mut hooked = false;
            let mut free = false;
            if (way & self.piece(opponent_color)) == 0 {
                add_eval(color, &mut eval_info.score, tables.passed_pawn_clear[r]);
                if (tables.pawn_paths[opponent_color][square_index] & self.major(color)) != 0 {
                    let square = next_bit(
                        opponent_color,
                        tables.pawn_paths[opponent_color][square_index] & self.major(color),
                    );
                    if (tables.between[square_index][square] & eval_info.occupancy) == 0 {
                        supported = true;
                    }
                }
                if (tables.pawn_paths[opponent_color][square_index] & self.major(opponent_color))
                    != 0
                {
                    let square = next_bit(
                        opponent_color,
                        tables.pawn_paths[opponent_color][square_index]
                            & self.major(opponent_color),
                    );
                    if (tables.between[square_index][square] & eval_info.occupancy) == 0 {
                        hooked = true;
                    }
                }
                let mut v = tables.pawn_attacks[color]
                    [(square_index as i32 - Self::push(color)) as usize]
                    & self.pawn(color);
                while v != 0 {
                    let square = least_significant_bit(v);
                    clear_lowest_bit(&mut v);
                    if (self.pawn(opponent_color)
                        & (tables.file[file(square)] | tables.isolated_pawns[file(square)])
                        & tables.forward_ranks[color][rank(square)])
                        == 0
                    {
                        connected += 1;
                    }
                }
                if connected != 0 {
                    add_eval(color, &mut eval_info.score, tables.passed_pawn_connected[r]);
                }
                if !hooked && (self.pos.states[current].attacks[opponent_color] & way) == 0 {
                    add_eval(color, &mut eval_info.score, tables.passed_pawn_free[r]);
                    free = true;
                } else {
                    let attacked = self.pos.states[current].attacks[opponent_color]
                        | if hooked { way } else { 0 };
                    if supported
                        || (!hooked && connected != 0)
                        || ((self.major(color) & way) == 0
                            && (attacked & (!self.pos.states[current].attacks[color])) == 0)
                    {
                        add_eval(color, &mut eval_info.score, tables.passed_pawn_supported[r]);
                    }
                }
            }
            if r == 6 {
                if (way & self.rook(color)) != 0
                    && self.minor(color) == 0
                    && self.queen(color) == 0
                    && has_single_bit(self.rook(color))
                {
                    subtract_eval(
                        color,
                        &mut eval_info.score,
                        compose(0, tables.passed_pawn_special[PASSER_OP_ROOK_BLOCK]),
                    );
                }
                if self.major(opponent_color) == 0
                    && (self.non_pawn_king(opponent_color) == 0
                        || has_single_bit(self.non_pawn_king(opponent_color)))
                {
                    add_eval(
                        color,
                        &mut eval_info.score,
                        compose(0, tables.passed_pawn_special[PASSER_ONE_PIECE]),
                    );
                    if !free {
                        if (tables.king_attack_area
                            [(square_index as i32 + Self::push(color)) as usize]
                            & self.king(opponent_color))
                            == 0
                        {
                            add_eval(
                                color,
                                &mut eval_info.score,
                                compose(0, tables.passed_pawn_special[PASSER_OP_MINOR_CONTROL]),
                            );
                        } else {
                            add_eval(
                                color,
                                &mut eval_info.score,
                                compose(0, tables.passed_pawn_special[PASSER_OP_KING_CONTROL]),
                            );
                        }
                    }
                }
            }
        }
    }

    #[inline(always)]
    fn eval_pieces(&mut self, color: Color, eval_info: &mut EvalInfo) {
        let tables = self.tables;
        let current = self.pos.current;
        let opponent_color = opponent(color);
        self.pos.states[current].threats |= self.pos.states[current].attacks[opponent_color]
            & (!self.pos.states[current].attacks[color])
            & self.piece(color);
        let mut u = self.pos.states[current].threats & self.piece(color);
        if u != 0 {
            subtract_eval(
                color,
                &mut eval_info.score,
                eval_pair(&tables.tactical, TACTICAL_THREAT),
            );
            u &= u - 1;
            if u != 0 {
                subtract_eval(
                    color,
                    &mut eval_info.score,
                    eval_pair(&tables.tactical, TACTICAL_THREAT)
                        + eval_pair(&tables.tactical, TACTICAL_DOUBLE_THREAT),
                );
                u &= u - 1;
                while u != 0 {
                    subtract_eval(
                        color,
                        &mut eval_info.score,
                        eval_pair(&tables.tactical, TACTICAL_THREAT),
                    );
                    u &= u - 1;
                }
            }
        }
    }

    fn eval_unusual_material(&mut self, eval_info: &EvalInfo) {
        let tables = self.tables;
        let current = self.pos.current;
        let white_pawn = population_count(self.pawn(WHITE));
        let black_pawn = population_count(self.pawn(BLACK));
        let wlight = population_count(self.minor(WHITE));
        let blight = population_count(self.minor(BLACK));
        let wr = population_count(self.rook(WHITE));
        let br = population_count(self.rook(BLACK));
        let wq = population_count(self.queen(WHITE));
        let bq = population_count(self.queen(BLACK));
        let phase = 24.min((wlight + blight) + 2 * (wr + br) + 4 * (wq + bq));
        let mat_score = SEE_VALUE[WHITE_PAWN] * (white_pawn - black_pawn)
            + SEE_VALUE[WHITE_KNIGHT] * (wlight - blight)
            + SEE_VALUE[WHITE_ROOK] * (wr - br)
            + SEE_VALUE[WHITE_QUEEN] * (wq - bq);
        let mat_score = compose(mat_score, mat_score);
        self.pos.states[current].score = (((opening(mat_score + eval_info.score) * phase)
            + (endgame(mat_score + eval_info.score) * (24 - phase)))
            / 24) as i16;
        if self.pos.states[current].side_to_move != 0 {
            self.pos.states[current].score = (-(self.pos.states[current].score as i32)) as i16;
        }
        let _ = tables;
        self.update_delta();
    }

    // -- endgame scaling ---------------------------------------------------

    #[inline(always)]
    fn eval_endgame(&mut self, color: Color, eval_info: &mut EvalInfo) {
        let tables = self.tables;
        let current = self.pos.current;
        let opponent_color = opponent(color);
        let material = eval_info.material;
        let flag = if color == WHITE {
            FLAG_SINGLE_BISHOP_WHITE
        } else {
            FLAG_SINGLE_BISHOP_BLACK
        };
        if (material.flags & flag) != 0 && self.pawn(color) != 0 {
            let square_index = if self.pos.board.bitboards[light_bishop_piece(color)] != 0 {
                if color == WHITE {
                    63
                } else {
                    0
                }
            } else if self.pos.board.bitboards[dark_bishop_piece(color)] != 0 {
                if color == WHITE {
                    56
                } else {
                    7
                }
            } else if file(least_significant_bit(self.king(opponent_color))) <= 3 {
                if color == WHITE {
                    56
                } else {
                    0
                }
            } else if color == WHITE {
                63
            } else {
                7
            };
            if (self.pawn(color) & (!tables.pawn_paths[opponent_color][square_index])) == 0 {
                if ((tables.king_attack_area[square_index] | bit(square_index))
                    & self.king(opponent_color))
                    != 0
                {
                    eval_info.evaluation_multiplier = 0;
                } else if (tables.king_attack_area[square_index]
                    & tables.king_attack_area[least_significant_bit(self.king(opponent_color))]
                    & self.line(color, 7))
                    != 0
                    && self.pos.board.squares[(square_index as i32 - Self::push(color)) as usize]
                        == pawn_piece(opponent_color) as u8
                    && self.pos.board.squares
                        [(square_index as i32 - 2 * Self::push(color)) as usize]
                        == pawn_piece(color) as u8
                {
                    eval_info.evaluation_multiplier = 0;
                }
            } else if ((self.king(opponent_color) & self.line(color, 6)) | self.line(color, 7)) != 0
                && (file(square_index) as i32
                    - file(least_significant_bit(self.king(opponent_color))) as i32)
                    .abs()
                    <= 3
                && (self.pawn(color) & (!tables.pawn_support[color][square_index])) == 0
                && (self.pawn(color)
                    & self.line(color, 5)
                    & Self::shift(opponent_color, self.pawn(opponent_color)))
                    != 0
            {
                eval_info.evaluation_multiplier = 0;
            }
            if has_single_bit(self.pawn(color)) {
                if self.bishop(color) == 0 {
                    eval_info.evaluation_multiplier = eval_info
                        .evaluation_multiplier
                        .min(self.king_pawn_vs_king(color));
                    if self.piece(opponent_color) == self.king(opponent_color)
                        && eval_info.evaluation_multiplier == 32
                    {
                        if color == WHITE {
                            self.pos.states[current].score =
                                (self.pos.states[current].score as i32 + KPK_VALUE) as i16;
                        } else {
                            self.pos.states[current].score =
                                (self.pos.states[current].score as i32 - KPK_VALUE) as i16;
                        }
                    }
                } else {
                    let square_index = least_significant_bit(self.pawn(color));
                    if (self.pawn(color) & (tables.file[1] | tables.file[6]) & self.line(color, 5))
                        != 0
                        && self.pos.board.squares
                            [(square_index as i32 + Self::push(color)) as usize]
                            == pawn_piece(opponent_color) as u8
                        && ((tables.pawn_attacks[color]
                            [(square_index as i32 + Self::push(color)) as usize]
                            | tables.pawn_paths[color]
                                [(square_index as i32 + Self::push(color)) as usize])
                            & self.king(opponent_color))
                            != 0
                    {
                        eval_info.evaluation_multiplier = 0;
                    }
                }
            }
            if self.bishop(opponent_color) != 0
                && has_single_bit(self.bishop(opponent_color))
                && ((self.pos.board.bitboards[light_bishop_piece(color)] != 0)
                    != (self.pos.board.bitboards[light_bishop_piece(opponent_color)] != 0))
            {
                let mut pcnt = 0i32;
                let king_opp = king_square(eval_info, opponent_color);
                let mut goto_partial = false;
                if ((self.king(opponent_color) & LIGHT_AREA) != 0)
                    == ((self.bishop(opponent_color) & LIGHT_AREA) != 0)
                {
                    let mut u = self.pawn(color);
                    while u != 0 {
                        if pcnt >= 2 {
                            goto_partial = true;
                            break;
                        }
                        pcnt += 1;
                        let square_index = least_significant_bit(u);
                        clear_lowest_bit(&mut u);
                        if (tables.pawn_paths[color][square_index]
                            & (tables.pawn_attacks[color][king_opp]
                                | tables.pawn_attacks[opponent_color][king_opp]))
                            == 0
                        {
                            if (tables.pawn_paths[color][square_index] & self.pawn(opponent_color))
                                == 0
                            {
                                goto_partial = true;
                                break;
                            }
                            let bishop_square = least_significant_bit(self.bishop(opponent_color));
                            let attacks = tables.bishop_attacks(bishop_square, eval_info.occupancy);
                            if (attacks
                                & tables.pawn_paths[color][square_index]
                                & self.pawn(opponent_color))
                                == 0
                            {
                                goto_partial = true;
                                break;
                            }
                            if (tables.bishop_forward_rays[color][bishop_square]
                                & attacks
                                & tables.pawn_paths[color][square_index]
                                & self.pawn(opponent_color))
                                == 0
                                && population_count(
                                    tables.full_line[least_significant_bit(
                                        attacks
                                            & tables.pawn_paths[color][square_index]
                                            & self.pawn(opponent_color),
                                    )][bishop_square]
                                        & attacks,
                                ) <= 2
                            {
                                goto_partial = true;
                                break;
                            }
                        }
                    }
                    if !goto_partial {
                        eval_info.evaluation_multiplier = 0;
                        return;
                    }
                }
                if pcnt <= 2
                    && has_multiple_bits(self.pawn(color))
                    && self.pawn(opponent_color) == 0
                    && (self.pawn(color) & BOUNDARY) == 0
                    && eval_info.evaluation_multiplier != 0
                {
                    let sq1 = least_significant_bit(self.pawn(color));
                    let sq2 = most_significant_bit(self.pawn(color));
                    let fd = (file(sq2) as i32 - file(sq1) as i32).abs();
                    if fd >= 5 {
                        eval_info.evaluation_multiplier = 32;
                    } else if fd >= 4 {
                        eval_info.evaluation_multiplier = 26;
                    } else if fd >= 3 {
                        eval_info.evaluation_multiplier = 20;
                    }
                }
                if ((tables.king_attack_area[king_opp]
                    | self.pos.states[current].pawn_attacks[opponent_color])
                    & self.bishop(opponent_color))
                    != 0
                {
                    let push = Self::shift(color, self.pawn(color));
                    if (push
                        & (!(self.piece(opponent_color)
                            | self.pos.states[current].attacks[opponent_color])))
                        == 0
                        && (self.king(opponent_color)
                            & if self.pos.board.bitboards[light_bishop_piece(opponent_color)] != 0 {
                                LIGHT_AREA
                            } else {
                                DARK_AREA
                            })
                            != 0
                    {
                        eval_info.evaluation_multiplier = eval_info.evaluation_multiplier.min(8);
                        let bishop_square = least_significant_bit(self.bishop(opponent_color));
                        let attacks = tables.bishop_attacks(bishop_square, eval_info.occupancy);
                        let prp = (attacks | tables.king_attack_area[king_opp])
                            & self.pawn(opponent_color)
                            & if self.pos.board.bitboards[light_bishop_piece(opponent_color)] != 0 {
                                LIGHT_AREA
                            } else {
                                DARK_AREA
                            };
                        let pawn_attacks = Self::shift_west(opponent_color, prp)
                            | Self::shift_east(opponent_color, prp);
                        if ((tables.king_attack_area[king_opp] | pawn_attacks)
                            & self.bishop(opponent_color))
                            != 0
                        {
                            let double_pawn_attacks_of = (tables.king_attack_area[king_opp]
                                & pawn_attacks)
                                | (pawn_attacks & attacks)
                                | (tables.king_attack_area[king_opp] & attacks);
                            if (push
                                & (!(self.king(opponent_color)
                                    | self.bishop(opponent_color)
                                    | prp
                                    | double_pawn_attacks_of)))
                                == 0
                            {
                                eval_info.evaluation_multiplier = 0;
                                return;
                            }
                        }
                    }
                }
            }
        }
        if self.major(color) == 0 {
            if self.bishop(color) != 0
                && self.knight(color) == 0
                && has_single_bit(self.bishop(color))
                && self.pawn(color) != 0
            {
                let number = population_count(self.pawn(color));
                if number == 1 {
                    if self.bishop(opponent_color) != 0 {
                        eval_info.evaluation_multiplier = eval_info
                            .evaluation_multiplier
                            .min(self.king_bishop_pawn_vs_king_bishop(color));
                    } else if self.knight(opponent_color) != 0 {
                        eval_info.evaluation_multiplier = eval_info
                            .evaluation_multiplier
                            .min(self.king_bishop_pawn_vs_king_knight(color));
                    }
                } else if number == 2 && self.bishop(opponent_color) != 0 {
                    eval_info.evaluation_multiplier = eval_info
                        .evaluation_multiplier
                        .min(self.king_bishop_two_pawns_vs_king_bishop(color));
                }
            } else if self.bishop(color) == 0
                && self.knight(color) != 0
                && has_single_bit(self.knight(color))
                && self.pawn(color) != 0
                && has_single_bit(self.pawn(color))
            {
                eval_info.evaluation_multiplier = eval_info
                    .evaluation_multiplier
                    .min(self.king_knight_pawn_vs_king(color));
            }
        } else if self.minor(color) == 0 {
            if self.pawn(color) == 0
                && self.rook(color) == 0
                && self.queen(color) != 0
                && self.pawn(opponent_color) != 0
            {
                if self.non_pawn_king(opponent_color) == 0
                    && has_single_bit(self.pawn(opponent_color))
                {
                    eval_info.evaluation_multiplier = eval_info
                        .evaluation_multiplier
                        .min(self.king_queen_vs_king_pawn(color));
                } else if self.rook(opponent_color) != 0 {
                    eval_info.evaluation_multiplier = eval_info
                        .evaluation_multiplier
                        .min(self.king_queen_vs_king_rook_pawn(color));
                }
            } else if self.queen(color) == 0
                && self.rook(color) != 0
                && has_single_bit(self.rook(color))
            {
                let number = population_count(self.pawn(color));
                if number <= 3 {
                    if number == 0 {
                        if self.pawn(opponent_color) != 0 {
                            eval_info.evaluation_multiplier = eval_info
                                .evaluation_multiplier
                                .min(self.king_rook_vs_king_pawn(color));
                        }
                    } else if self.rook(opponent_color) != 0 {
                        if number == 1 {
                            let new_mul = self.king_rook_pawn_vs_king_rook(color);
                            eval_info.evaluation_multiplier = if new_mul <= 32 {
                                eval_info.evaluation_multiplier.min(new_mul)
                            } else {
                                new_mul
                            };
                        } else {
                            if number == 2 {
                                eval_info.evaluation_multiplier = eval_info
                                    .evaluation_multiplier
                                    .min(self.king_rook_two_pawns_vs_king_rook(color));
                            }
                            if self.pawn(opponent_color) != 0 {
                                if number == 2 {
                                    eval_info.evaluation_multiplier = eval_info
                                        .evaluation_multiplier
                                        .min(self.king_rook_two_pawns_vs_king_rook_pawn(color));
                                } else if has_multiple_bits(self.pawn(opponent_color)) {
                                    eval_info.evaluation_multiplier =
                                        eval_info.evaluation_multiplier.min(
                                            self.king_rook_three_pawns_vs_king_rook_two_pawns(
                                                color,
                                            ),
                                        );
                                }
                            }
                        }
                    } else if number == 1 && self.bishop(opponent_color) != 0 {
                        eval_info.evaluation_multiplier = eval_info
                            .evaluation_multiplier
                            .min(self.king_rook_pawn_vs_king_bishop(color));
                    }
                }
            }
        } else if self.pawn(color) == 0
            && has_single_bit(self.rook(color))
            && self.queen(color) == 0
            && has_single_bit(self.bishop(color))
            && self.knight(color) == 0
            && self.rook(opponent_color) != 0
        {
            eval_info.evaluation_multiplier = eval_info
                .evaluation_multiplier
                .min(self.king_rook_bishop_vs_king_rook(color));
        }
        if self.non_pawn_king(opponent_color) == 0
            && self.pos.states[current].side_to_move == opponent_color as u8
            && (self.pos.states[current].attacks[color] & self.king(opponent_color)) == 0
            && (tables.king_attack_area[king_square(eval_info, opponent_color)]
                & (!(self.pos.states[current].attacks[color] | self.piece(opponent_color))))
                == 0
            && (self.pos.states[current].pawn_attacks[opponent_color] & self.piece(color)) == 0
            && (Self::shift(opponent_color, self.pawn(opponent_color)) & (!eval_info.occupancy))
                == 0
        {
            eval_info.evaluation_multiplier = 0;
        }
    }

    fn king_pawn_vs_king(&self, color: Color) -> i32 {
        let tables = self.tables;
        let side_to_move = self.pos.states[self.pos.current].side_to_move as usize;
        let u = if color == WHITE {
            tables.kpk(
                side_to_move,
                least_significant_bit(self.pawn(WHITE)),
                least_significant_bit(self.king(WHITE)),
            ) & bit(least_significant_bit(self.king(BLACK)))
        } else {
            tables.kpk(
                side_to_move ^ 1,
                63 - least_significant_bit(self.pawn(BLACK)),
                63 - least_significant_bit(self.king(BLACK)),
            ) & bit(63 - least_significant_bit(self.king(WHITE)))
        };
        if u != 0 {
            32
        } else if (self.piece(opponent(color)) ^ self.king(opponent(color))) != 0 {
            1
        } else {
            0
        }
    }

    fn king_knight_pawn_vs_king(&self, color: Color) -> i32 {
        let tables = self.tables;
        if (self.pawn(color) & self.line(color, 6) & (tables.file[0] | tables.file[7])) != 0 {
            let square_index = least_significant_bit(self.pawn(color));
            if (tables.king_attack_area[square_index]
                & self.king(opponent(color))
                & (self.line(color, 6) | self.line(color, 7)))
                != 0
            {
                return 0;
            }
            let p = (square_index as i32 + Self::push(color)) as usize;
            if self.pos.board.squares[p] == king_piece(color) as u8
                && (tables.king_attack_area[least_significant_bit(self.king(color))]
                    & tables.king_attack_area[least_significant_bit(self.king(opponent(color)))]
                    & self.line(color, 7))
                    != 0
            {
                return 0;
            }
        } else if (self.pawn(color) & self.line(color, 5) & (tables.file[0] | tables.file[7])) != 0
        {
            let square_index = least_significant_bit(self.pawn(color));
            let p = (square_index as i32 + Self::push(color)) as usize;
            if self.pos.board.squares[p] == pawn_piece(opponent(color)) as u8 {
                if (tables.king_attack_area[p] & self.king(opponent(color)) & self.line(color, 7))
                    != 0
                {
                    return 0;
                }
                if (tables.king_attack_area[p]
                    & tables.king_attack_area[least_significant_bit(self.king(opponent(color)))]
                    & self.line(color, 7))
                    != 0
                    && ((tables.knight_attacks[p] & self.knight(color)) == 0
                        || self.pos.states[self.pos.current].side_to_move as Color
                            == opponent(color))
                {
                    return 0;
                }
            }
        }
        32
    }

    fn king_rook_pawn_vs_king_rook(&self, color: Color) -> i32 {
        let tables = self.tables;
        let current = self.pos.current;
        let opponent_color = opponent(color);
        let mut mul = 32;
        let square_index = least_significant_bit(self.pawn(color));
        let relative_rank_of_pawn = relative_rank(color, square_index);
        let opponent_king = least_significant_bit(self.king(opponent_color));
        let opponent_rook = least_significant_bit(self.rook(opponent_color));
        let our_king = least_significant_bit(self.king(color));
        let add_mat =
            (self.piece(opponent_color) ^ self.king(opponent_color) ^ self.rook(opponent_color))
                != 0;
        let clear = !add_mat
            || ((tables.pawn_paths[opponent_color][square_index]
                | tables.isolated_pawns[file(square_index)])
                & tables.forward_ranks[opponent_color]
                    [rank((square_index as i32 + Self::push(color)) as usize)]
                & (self.piece(opponent_color)
                    ^ self.king(opponent_color)
                    ^ self.rook(opponent_color)))
                == 0;
        if !clear {
            return 32;
        }
        if !add_mat && (self.pawn(color) & (tables.file[0] | tables.file[7])) == 0 {
            let our_rook = least_significant_bit(self.rook(color));
            if relative_rank(color, opponent_king) < relative_rank(color, our_rook)
                && relative_rank(color, our_rook) < relative_rank_of_pawn
                && relative_rank(color, our_king) >= relative_rank_of_pawn.wrapping_sub(1)
                && relative_rank(color, our_king) > relative_rank(color, our_rook)
                && ((tables.king_attack_area[our_king] & self.pawn(color)) != 0
                    || (self.pos.states[current].side_to_move as Color == color
                        && (file(square_index) as i32 - file(our_king) as i32).abs() <= 1
                        && (relative_rank_of_pawn as i32 - relative_rank(color, our_king) as i32)
                            .abs()
                            <= 2))
            {
                return 128;
            }
            if (tables.king_attack_area[our_king] & self.pawn(color)) != 0 {
                if relative_rank_of_pawn >= 4 {
                    if (file(square_index) < file(our_rook) && file(our_rook) < file(opponent_king))
                        || (file(square_index) > file(our_rook)
                            && file(our_rook) > file(opponent_king))
                    {
                        return 128;
                    }
                } else if relative_rank_of_pawn >= 2 {
                    if (self.pawn(color) & (tables.file[1] | tables.file[6])) == 0
                        && relative_rank_of_pawn
                            + (file(square_index) as i32 - file(our_rook) as i32).abs() as usize
                            > 4
                        && ((file(square_index) < file(our_rook)
                            && file(our_rook) < file(opponent_king))
                            || (file(square_index) > file(our_rook)
                                && file(our_rook) > file(opponent_king)))
                    {
                        return 128;
                    }
                }
            }
        }
        if (tables.pawn_paths[color][square_index] & self.king(opponent_color)) != 0 {
            if (self.pawn(color) & (tables.file[0] | tables.file[7])) != 0 {
                mul = mul.min(if add_mat { 8 } else { 0 });
            }
            if relative_rank_of_pawn <= 3 {
                mul = mul.min(if add_mat { 8 } else { 0 });
            }
            if relative_rank_of_pawn == 4
                && relative_rank(color, our_king) <= 4
                && relative_rank(color, opponent_rook) == 5
                && (self.king(opponent_color) & (self.line(color, 6) | self.line(color, 7))) != 0
                && (self.pos.states[current].side_to_move as Color != color
                    || (tables.pawn_attacks[color][square_index]
                        & tables.rook_attacks(
                            least_significant_bit(self.rook(color)),
                            self.piece_all(),
                        )
                        & (!tables.king_attack_area[opponent_king]))
                        == 0)
            {
                mul = mul.min(if add_mat { 8 } else { 0 });
            }
            if relative_rank_of_pawn >= 5
                && relative_rank(color, opponent_rook) <= 1
                && (self.pos.states[current].side_to_move as Color != color
                    || self.check(color)
                    || distance(our_king, square_index) >= 2)
            {
                mul = mul.min(if add_mat { 8 } else { 0 });
            }
            if (self.king(opponent_color)
                & (tables.file[1] | tables.file[2] | tables.file[6] | tables.file[7]))
                != 0
                && (self.rook(opponent_color) & self.line(color, 7)) != 0
                && (tables.between[opponent_king][opponent_rook]
                    & (tables.file[3] | tables.file[4]))
                    != 0
                && (self.rook(color) & self.line(color, 7)) == 0
            {
                mul = mul.min(if add_mat { 8 } else { 0 });
            }
            mul
        } else if relative_rank_of_pawn == 6
            && (self.pawn(color) & (tables.file[0] | tables.file[7])) != 0
            && ((tables.pawn_support[color][square_index]
                | tables.pawn_paths[opponent_color][square_index])
                & self.rook(opponent_color))
                != 0
            && relative_rank(color, opponent_king) >= 6
        {
            let d = (file(square_index) as i32 - file(opponent_king) as i32).abs();
            if d <= 3 {
                mul = mul.min(if add_mat { 8 } else { 0 });
            }
            if d == 4
                && ((tables.pawn_support[color][opponent_king] & self.rook(color)) != 0
                    || self.pos.states[current].side_to_move as Color == opponent_color)
            {
                mul = mul.min(if add_mat { 8 } else { 0 });
            }
            mul
        } else {
            if (tables.king_attack_area[opponent_king]
                & tables.pawn_paths[color][square_index]
                & self.line(color, 7))
                != 0
            {
                if relative_rank_of_pawn <= 4
                    && relative_rank(color, our_king) <= 4
                    && relative_rank(color, opponent_rook) == 5
                {
                    mul = mul.min(if add_mat { 8 } else { 0 });
                }
                if (relative_rank_of_pawn == 5
                    && relative_rank(color, opponent_rook) <= 1
                    && self.pos.states[current].side_to_move as Color != color)
                    || ((tables.king_attack_area[our_king]
                        & tables.pawn_attacks[color][square_index]
                        & (!tables.king_attack_area[opponent_king]))
                        == 0
                        && (self.check(color) || distance(our_king, square_index) >= 2))
                {
                    mul = mul.min(if add_mat { 8 } else { 0 });
                }
            }
            if (tables.pawn_paths[color][square_index] & self.rook(color)) != 0
                && (tables.pawn_paths[opponent_color][square_index] & self.rook(opponent_color))
                    != 0
            {
                if (self.king(opponent_color)
                    & (tables.file[0] | tables.file[1] | tables.file[6] | tables.file[7])
                    & self.line(color, 6))
                    != 0
                {
                    mul = mul.min(if add_mat { 8 } else { 0 });
                } else if (self.pawn(color) & (tables.file[0] | tables.file[7])) != 0
                    && (self.king(opponent_color) & (self.line(color, 5) | self.line(color, 6)))
                        != 0
                    && (file(square_index) as i32 - file(opponent_king) as i32).abs() <= 2
                    && file(square_index) != file(opponent_king)
                {
                    mul = mul.min(if add_mat { 8 } else { 0 });
                }
            }
            if (file(square_index) as i32 - file(opponent_king) as i32).abs() <= 1
                && (file(square_index) as i32 - file(opponent_rook) as i32).abs() <= 1
                && relative_rank(color, opponent_rook) > relative_rank_of_pawn
                && relative_rank(color, opponent_king) > relative_rank_of_pawn
            {
                mul = mul.min(
                    if (self.pawn(color) & (tables.file[3] | tables.file[4])) != 0 {
                        12
                    } else {
                        16
                    },
                );
            }
            mul
        }
    }

    fn king_rook_pawn_vs_king_bishop(&self, color: Color) -> i32 {
        let tables = self.tables;
        let current = self.pos.current;
        let opponent_color = opponent(color);
        if (self.pawn(color) & self.line(color, 5)) == 0 {
            return 32;
        }
        let square_index = least_significant_bit(self.pawn(color));
        if (tables.pawn_paths[color][square_index] & self.king(opponent_color)) == 0 {
            return 32;
        }
        let p = (square_index as i32 + Self::push(color)) as usize;
        let diag_sq = next_bit(color, tables.bishop_attack_mask[p]);
        if relative_rank(color, diag_sq) > 1 {
            return 32;
        }
        let mdiag = tables.full_line[p][diag_sq] | bit(p) | bit(diag_sq);
        let q = (square_index as i32 - Self::push(color)) as usize;
        let check_sq = next_bit(color, tables.bishop_attack_mask[q]);
        let cdiag = tables.full_line[q][check_sq] | bit(q) | bit(check_sq);
        if ((mdiag | cdiag)
            & (self.piece(opponent_color)
                ^ self.king(opponent_color)
                ^ self.bishop(opponent_color)))
            != 0
        {
            return 32;
        }
        if (cdiag & self.bishop(opponent_color)) != 0 {
            return 0;
        }
        if (mdiag & self.bishop(opponent_color)) != 0
            && (self.pos.states[current].side_to_move as Color == opponent_color
                || (self.king(color) & tables.pawn_attacks[opponent_color][p]) == 0)
        {
            return 0;
        }
        32
    }

    fn king_queen_vs_king_pawn(&self, color: Color) -> i32 {
        let tables = self.tables;
        let opponent_color = opponent(color);
        if (tables.king_attack_area[least_significant_bit(self.king(opponent_color))]
            & self.pawn(opponent_color)
            & self.line(color, 1)
            & (tables.file[0] | tables.file[2] | tables.file[5] | tables.file[7]))
            == 0
        {
            return 32;
        }
        if (tables.pawn_paths[opponent_color][least_significant_bit(self.pawn(opponent_color))]
            & (self.king(color) | self.queen(color)))
            != 0
        {
            return 32;
        }
        if (self.pawn(opponent_color) & (tables.file[0] | tables.file[7])) != 0 {
            1
        } else {
            4
        }
    }

    fn king_queen_vs_king_rook_pawn(&self, color: Color) -> i32 {
        let tables = self.tables;
        let opponent_color = opponent(color);
        let rook_square = least_significant_bit(self.rook(opponent_color));
        let pawns = tables.king_attack_area[least_significant_bit(self.king(opponent_color))]
            & tables.pawn_attacks[color][rook_square]
            & self.pawn(opponent_color)
            & INTERIOR
            & self.line(color, 6);
        if pawns != 0 && relative_rank(color, least_significant_bit(self.king(color))) <= 4 {
            return 0;
        }
        32
    }

    fn king_rook_vs_king_pawn(&self, color: Color) -> i32 {
        let tables = self.tables;
        let opponent_color = opponent(color);
        if (tables.king_attack_area[least_significant_bit(self.king(opponent_color))]
            & self.pawn(opponent_color)
            & self.line(color, 1))
            != 0
            && (tables.pawn_paths[opponent_color][next_bit(color, self.pawn(opponent_color))]
                & self.king(color))
                == 0
        {
            return 0;
        }
        32
    }

    fn king_rook_two_pawns_vs_king_rook_pawn(&self, color: Color) -> i32 {
        let tables = self.tables;
        let current = self.pos.current;
        let opponent_color = opponent(color);
        if (self.pos.states[current].passed_pawns & self.pawn(color)) != 0 {
            if has_single_bit(self.pos.states[current].passed_pawns & self.pawn(color)) {
                let square_index =
                    least_significant_bit(self.pos.states[current].passed_pawns & self.pawn(color));
                if (tables.pawn_paths[color][square_index]
                    & self.king(opponent_color)
                    & (tables.file[0] | tables.file[1] | tables.file[6] | tables.file[7]))
                    != 0
                {
                    let opp_king = least_significant_bit(self.king(opponent_color));
                    if (tables.king_attack_area[opp_king] & self.pawn(opponent_color)) != 0 {
                        let king_file = file(opp_king);
                        if ((!(tables.file[king_file] | tables.isolated_pawns[king_file]))
                            & self.pawn(color))
                            != 0
                        {
                            return 1;
                        }
                    }
                }
            }
            return 32;
        }
        if ((!(tables.pawn_paths[opponent_color]
            [least_significant_bit(self.king(opponent_color))]
            | tables.pawn_support[color][least_significant_bit(self.king(opponent_color))]))
            & self.pawn(color))
            == 0
        {
            return 0;
        }
        32
    }

    fn king_rook_three_pawns_vs_king_rook_two_pawns(&self, color: Color) -> i32 {
        let tables = self.tables;
        let opponent_color = opponent(color);
        if (self.pos.states[self.pos.current].passed_pawns & self.pawn(color)) != 0
            || (tables.king_attack_area[least_significant_bit(self.pawn(opponent_color))]
                | tables.king_attack_area[most_significant_bit(self.pawn(opponent_color))])
                & self.pawn(opponent_color)
                == 0
        {
            return 32;
        }
        if ((!(tables.pawn_paths[opponent_color]
            [least_significant_bit(self.king(opponent_color))]
            | tables.pawn_support[color][least_significant_bit(self.king(opponent_color))]))
            & self.pawn(color))
            == 0
        {
            return 0;
        }
        32
    }

    fn king_bishop_pawn_vs_king_bishop(&self, color: Color) -> i32 {
        let tables = self.tables;
        let current = self.pos.current;
        let opponent_color = opponent(color);
        let square_index = least_significant_bit(self.pawn(color));
        if (self.pos.board.bitboards[light_bishop_piece(color)] != 0
            && self.pos.board.bitboards[dark_bishop_piece(opponent_color)] != 0)
            || (self.pos.board.bitboards[dark_bishop_piece(color)] != 0
                && self.pos.board.bitboards[light_bishop_piece(opponent_color)] != 0)
        {
            if relative_rank(color, square_index) <= 4 {
                return 0;
            }
            if (tables.pawn_paths[color][square_index] & self.king(opponent_color)) != 0
                && relative_rank(color, square_index) <= 5
            {
                return 0;
            }
            let mut u = self.bishop(opponent_color);
            while u != 0 {
                let s = least_significant_bit(u);
                clear_lowest_bit(&mut u);
                if relative_rank(color, s) <= 4
                    && (tables.bishop_attacks(s, self.piece_all())
                        & tables.pawn_paths[color][square_index])
                        != 0
                {
                    return 0;
                }
                if self.pos.states[current].side_to_move as Color == opponent_color
                    && (tables.bishop_attacks(s, self.piece_all()) & self.pawn(color)) != 0
                {
                    return 0;
                }
            }
        } else if (tables.pawn_paths[color][square_index] & self.king(opponent_color)) != 0
            && ((self.king(opponent_color) & LIGHT_AREA) != 0)
                != ((self.bishop(color) & LIGHT_AREA) != 0)
        {
            return 0;
        }
        32
    }

    fn king_bishop_pawn_vs_king_knight(&self, color: Color) -> i32 {
        let tables = self.tables;
        let current = self.pos.current;
        let opponent_color = opponent(color);
        if (tables.pawn_paths[color][least_significant_bit(self.pawn(color))]
            & self.king(opponent_color))
            != 0
            && ((self.king(opponent_color) & LIGHT_AREA) != 0)
                != ((self.bishop(color) & LIGHT_AREA) != 0)
        {
            return 0;
        }
        if self.pos.states[current].side_to_move as Color == opponent_color {
            let mut u = self.knight(opponent_color);
            while u != 0 {
                let s = least_significant_bit(u);
                clear_lowest_bit(&mut u);
                if (tables.knight_attacks[s] & self.pawn(color)) != 0 {
                    return 0;
                }
            }
        }
        32
    }

    fn king_bishop_two_pawns_vs_king_bishop(&self, color: Color) -> i32 {
        let tables = self.tables;
        let opponent_color = opponent(color);
        let sq1 = next_bit(color, self.pawn(color));
        let sq2 = next_bit(opponent_color, self.pawn(color));
        let opponent_king = least_significant_bit(self.king(opponent_color));
        let o_bishop = least_significant_bit(self.bishop(opponent_color));
        if file(sq1) == file(sq2) {
            if relative_rank(color, sq2) <= 3 {
                return 0;
            }
            if (tables.pawn_paths[color][sq2] & self.king(opponent_color)) != 0
                && relative_rank(color, sq2) <= 5
            {
                return 0;
            }
        } else if (tables.isolated_pawns[file(sq1)] & self.pawn(color)) != 0 {
            if ((self.king(opponent_color) & LIGHT_AREA) != 0)
                != ((self.bishop(color) & LIGHT_AREA) != 0)
            {
                let target = (sq2 as i32 + Self::push(color)) as usize;
                if ((tables.king_attack_area[opponent_king] | self.king(opponent_color))
                    & bit(target))
                    != 0
                    && (tables.bishop_attacks(o_bishop, self.piece_all()) & bit(target)) != 0
                {
                    let target2 = (sq2 & !7usize) | file(sq1);
                    if ((tables.king_attack_area[opponent_king] | self.king(opponent_color))
                        & bit(target2))
                        != 0
                        && (tables.bishop_attacks(o_bishop, self.piece_all()) & bit(target2)) != 0
                    {
                        return 0;
                    }
                }
            }
        }
        32
    }

    fn king_rook_two_pawns_vs_king_rook(&self, color: Color) -> i32 {
        let tables = self.tables;
        let opponent_color = opponent(color);
        let sq1 = next_bit(color, self.pawn(color));
        let sq2 = next_bit(opponent_color, self.pawn(color));
        if ((self.piece(opponent_color) ^ self.king(opponent_color) ^ self.rook(opponent_color))
            & tables.forward_ranks[color][rank((sq1 as i32 - Self::push(color)) as usize)])
            != 0
        {
            return 32;
        }
        if file(sq1) == file(sq2) {
            if (tables.pawn_paths[color][sq2] & self.king(opponent_color)) != 0 {
                return 16;
            }
            return 32;
        }
        if (tables.isolated_pawns[file(sq2)] & self.pawn(color)) != 0
            && ((tables.file[0] | tables.file[7]) & self.pawn(color)) != 0
            && (self.king(opponent_color) & Self::shift(color, self.pawn(color))) != 0
        {
            if relative_rank(color, sq2) == 5
                && relative_rank(color, sq1) == 4
                && (self.rook(opponent_color) & (self.line(color, 5) | self.line(color, 6))) != 0
            {
                return 10;
            } else if relative_rank(color, sq2) < 5 {
                return 16;
            }
        }
        32
    }

    fn king_rook_bishop_vs_king_rook(&self, color: Color) -> i32 {
        let _ = color;
        if (self.king(opponent(color)) & INTERIOR) != 0 {
            1
        } else {
            16
        }
    }
}

// ---------------------------------------------------------------------------
// Pawn structure evaluation
// ---------------------------------------------------------------------------

fn eval_pawn_structure(pos: &Position, tables: &Tables, pe: &mut PawnEntry) {
    *pe = PawnEntry::default();
    pe.key = (pos.states[pos.current].pawn_key >> 16) & 0x0000_FFFF_FFFF_FFFF;
    let mut pei = PawnEvalInfo {
        white_king: least_significant_bit(pos.board.bitboards[WHITE_KING]),
        black_king: least_significant_bit(pos.board.bitboards[BLACK_KING]),
        score: 0,
        white_pawn_attacks: Engine::shift_west(WHITE, pos.board.bitboards[WHITE_PAWN])
            | Engine::shift_east(WHITE, pos.board.bitboards[WHITE_PAWN]),
        black_pawn_attacks: Engine::shift_west(BLACK, pos.board.bitboards[BLACK_PAWN])
            | Engine::shift_east(BLACK, pos.board.bitboards[BLACK_PAWN]),
        white_double_pawn_attacks: Engine::shift_west(WHITE, pos.board.bitboards[WHITE_PAWN])
            & Engine::shift_east(WHITE, pos.board.bitboards[WHITE_PAWN]),
        black_double_pawn_attacks: Engine::shift_west(BLACK, pos.board.bitboards[BLACK_PAWN])
            & Engine::shift_east(BLACK, pos.board.bitboards[BLACK_PAWN]),
    };
    eval_pawns(WHITE, pos, tables, pe, &mut pei);
    eval_pawns(BLACK, pos, tables, pe, &mut pei);
    pe.score = pei.score;
}

#[inline(always)]
fn pawn_attacks_of(eval_info: &PawnEvalInfo, color: Color) -> u64 {
    if color == WHITE {
        eval_info.white_pawn_attacks
    } else {
        eval_info.black_pawn_attacks
    }
}

#[inline(always)]
fn double_pawn_attacks_of(eval_info: &PawnEvalInfo, color: Color) -> u64 {
    if color == WHITE {
        eval_info.white_double_pawn_attacks
    } else {
        eval_info.black_double_pawn_attacks
    }
}

#[inline(always)]
fn pawn_eval_king_square(eval_info: &PawnEvalInfo, color: Color) -> usize {
    if color == WHITE {
        eval_info.white_king
    } else {
        eval_info.black_king
    }
}

fn eval_pawns(
    color: Color,
    pos: &Position,
    tables: &Tables,
    pe: &mut PawnEntry,
    pei: &mut PawnEvalInfo,
) {
    let opponent_color = opponent(color);
    let board = &pos.board;
    let own_pawns = board.bitboards[WHITE_PAWN | color];
    let opponent_pawns = board.bitboards[WHITE_PAWN | opponent_color];
    let all_pawns = board.bitboards[WHITE_PAWN] | board.bitboards[BLACK_PAWN];
    let king_file = file(pawn_eval_king_square(pei, color));
    let king_rank = rank(pawn_eval_king_square(pei, color));
    let (start, inc) = if king_file <= 3 {
        (king_file.saturating_sub(1), 1i32)
    } else {
        ((king_file + 1).min(7), -1i32)
    };
    let mut shelter = 0i32;
    let most_advanced_pawns = own_pawns
        & tables.forward_ranks[color][if color == BLACK {
            (king_rank + 1).min(7)
        } else {
            king_rank.saturating_sub(1)
        }];
    let mut f = start as i32;
    for i in 0..3 {
        let file_idx = f as usize;
        shelter += tables.shelter[i][relative_rank(
            color,
            next_bit_or_zero(color, most_advanced_pawns & tables.file[file_idx]),
        )];
        if (opponent_pawns & tables.file[file_idx]) != 0 {
            let square_index = next_bit(color, opponent_pawns & tables.file[file_idx]);
            let r = relative_rank(opponent_color, square_index);
            if r < 6 {
                if r >= 3 {
                    shelter += tables.storm_blocked[r - 3];
                }
                let u = tables.isolated_pawns[file(square_index)]
                    & tables.forward_ranks[opponent_color][rank(square_index)]
                    & own_pawns;
                if u != 0 {
                    let square = next_bit(opponent_color, u);
                    let attack_square = tables.pawn_attacks[color][square]
                        & tables.pawn_paths[opponent_color][square_index];
                    if ((tables.file[file(square)] | tables.isolated_pawns[file(square)])
                        & board.bitboards[WHITE_KING | color])
                        != 0
                        && ((double_pawn_attacks_of(pei, color) & attack_square) == 0
                            || (pos.states[pos.current].pawn_attacks[opponent_color]
                                & attack_square)
                                != 0)
                    {
                        if (tables.pawn_paths[opponent_color][square] & own_pawns) != 0 {
                            f += inc;
                            continue;
                        }
                        if (all_pawns
                            & tables.pawn_paths[opponent_color][square_index]
                            & tables.forward_ranks[color][rank(square)])
                            == 0
                        {
                            if r >= 3 {
                                shelter += tables.storm_shelter_att[r - 3];
                                if (pawn_attacks_of(pei, opponent_color)
                                    & bit((square_index as i32 + Engine::push(opponent_color))
                                        as usize))
                                    != 0
                                {
                                    shelter += tables.storm_connected[r - 3];
                                }
                                if (tables.pawn_paths[opponent_color][square_index] & all_pawns)
                                    == 0
                                {
                                    shelter += tables.storm_open[r - 3];
                                }
                            }
                            if ((tables.file[file(square_index)]
                                | tables.isolated_pawns[file(square_index)])
                                & board.bitboards[WHITE_KING | opponent_color])
                                == 0
                                && r <= 4
                            {
                                shelter += tables.storm_free[r - 1];
                            }
                        }
                    }
                }
            }
        } else {
            shelter += tables.storm_hof[STORM_HOF_VALUE];
            if (own_pawns & tables.file[file_idx]) == 0 {
                shelter += tables.storm_hof[STORM_OF_VALUE];
            }
        }
        f += inc;
    }
    pe.shelter[color] = if shelter > 255 { 255 } else { shelter as u8 };
    pe.passed_pawns[color] = 0;

    let mut u = own_pawns;
    while u != 0 {
        let square_index = least_significant_bit(u);
        let b = bit(square_index);
        u ^= b;
        let r = rank(square_index);
        let relative_rank_of_pawn = relative_rank(color, square_index);
        let f = file(square_index);
        let way = tables.pawn_paths[color][square_index];
        let next = board.squares[(square_index as i32 + Engine::push(color)) as usize] as usize;
        let isolated = (own_pawns & tables.isolated_pawns[f]) == 0;
        let doubled = (own_pawns & (tables.file[f] ^ b)) != 0;
        let open = (all_pawns & way) == 0;
        let up = (pawn_attacks_of(pei, color) & b) == 0;

        if isolated {
            if open {
                subtract_eval(
                    color,
                    &mut pei.score,
                    eval_pair(&tables.isolated, ISOLATED_OPEN),
                );
            } else {
                subtract_eval(
                    color,
                    &mut pei.score,
                    eval_pair(&tables.isolated, ISOLATED_CLOSED),
                );
                if next == pawn_piece(opponent_color) {
                    subtract_eval(
                        color,
                        &mut pei.score,
                        eval_pair(&tables.isolated, ISOLATED_BLOCKED),
                    );
                }
            }
            if doubled {
                if open {
                    subtract_eval(
                        color,
                        &mut pei.score,
                        eval_pair(&tables.isolated, ISOLATED_DOUBLED_OPEN),
                    );
                } else {
                    subtract_eval(
                        color,
                        &mut pei.score,
                        eval_pair(&tables.isolated, ISOLATED_DOUBLED_CLOSED),
                    );
                }
            }
        } else {
            if doubled {
                if open {
                    subtract_eval(
                        color,
                        &mut pei.score,
                        eval_pair(&tables.doubled, DOUBLED_OPEN),
                    );
                } else {
                    subtract_eval(
                        color,
                        &mut pei.score,
                        eval_pair(&tables.doubled, DOUBLED_CLOSED),
                    );
                }
            }
            if relative_rank_of_pawn >= 3
                && (b & (tables.file[2] | tables.file[3] | tables.file[4] | tables.file[5])) != 0
                && next != pawn_piece(opponent_color)
                && (tables.isolated_pawns[f] & tables.line[r] & own_pawns) != 0
            {
                add_eval(
                    color,
                    &mut pei.score,
                    eval_pair(&tables.pawn_special, PAWN_CHAIN_LINEAR)
                        * (relative_rank_of_pawn as i32 - 3)
                        + eval_pair(&tables.pawn_special, PAWN_CHAIN),
                );
            }
        }
        let mut backward = false;
        if (tables.pawn_support[color][square_index] & own_pawns) == 0 {
            if isolated {
                backward = true;
            } else {
                let v = (all_pawns | pawn_attacks_of(pei, opponent_color)) & way;
                if v != 0
                    && Engine::is_greater(
                        color,
                        next_bit(color, pawn_attacks_of(pei, color) & way),
                        next_bit(color, v),
                    )
                {
                    backward = true;
                }
            }
        }
        if backward {
            if open {
                subtract_eval(
                    color,
                    &mut pei.score,
                    eval_pair(&tables.backward, BACKWARD_OPEN),
                );
            } else {
                subtract_eval(
                    color,
                    &mut pei.score,
                    eval_pair(&tables.backward, BACKWARD_CLOSED),
                );
            }
        } else if open
            && ((opponent_pawns & tables.isolated_pawns[f]) == 0
                || population_count(own_pawns & tables.isolated_pawns[f])
                    >= population_count(opponent_pawns & tables.isolated_pawns[f]))
        {
            add_eval(
                color,
                &mut pei.score,
                tables.passed_pawn_candidate[relative_rank_of_pawn],
            );
        }
        if up && next == pawn_piece(opponent_color) {
            subtract_eval(
                color,
                &mut pei.score,
                eval_pair(&tables.unprotected, UP_BLOCKED),
            );
            if backward {
                if relative_rank_of_pawn <= 2 {
                    subtract_eval(
                        color,
                        &mut pei.score,
                        eval_pair(&tables.unprotected, PASSER_TARGET),
                    );
                    if relative_rank_of_pawn <= 1 {
                        subtract_eval(
                            color,
                            &mut pei.score,
                            eval_pair(&tables.unprotected, PASSER_TARGET),
                        );
                    }
                }
                let mut v = tables.pawn_attacks[color][square_index] & own_pawns;
                while v != 0 {
                    let s = least_significant_bit(v);
                    clear_lowest_bit(&mut v);
                    if (tables.pawn_support[color][s] & own_pawns) == b {
                        subtract_eval(
                            color,
                            &mut pei.score,
                            eval_pair(&tables.unprotected, CHAIN_ROOT),
                        );
                        break;
                    }
                }
            }
        }
        if open && (tables.isolated_pawns[f] & tables.forward_ranks[color][r] & opponent_pawns) == 0
        {
            pe.passed_pawns[color] |= 1u8 << f;
            if relative_rank_of_pawn <= 2 {
                continue;
            }
            add_eval(
                color,
                &mut pei.score,
                tables.passed_pawn_general[relative_rank_of_pawn],
            );
            let dist_att = distance(
                pawn_eval_king_square(pei, opponent_color),
                (square_index as i32 + Engine::push(color)) as usize,
            );
            let dist_def = distance(
                pawn_eval_king_square(pei, color),
                (square_index as i32 + Engine::push(color)) as usize,
            );
            add_eval(
                color,
                &mut pei.score,
                compose256(
                    0,
                    dist_att * tables.passed_pawn_att[relative_rank_of_pawn]
                        + tables.log_dist[dist_att as usize]
                            * tables.passed_pawn_att_log[relative_rank_of_pawn]
                        - dist_def * tables.passed_pawn_def[relative_rank_of_pawn]
                        - tables.log_dist[dist_def as usize]
                            * tables.passed_pawn_def_log[relative_rank_of_pawn],
                ),
            );
            if (pawn_attacks_of(pei, color) & b) != 0 {
                add_eval(
                    color,
                    &mut pei.score,
                    tables.passed_pawn_protected[relative_rank_of_pawn],
                );
            }
            if (opponent_pawns & tables.west_files[f]) == 0
                || (opponent_pawns & tables.east_files[f]) == 0
            {
                add_eval(
                    color,
                    &mut pei.score,
                    tables.passed_pawn_outside[relative_rank_of_pawn],
                );
            }
        }
    }
    let mut files = 0u64;
    for i in 1..7 {
        files |= (own_pawns >> (i << 3)) & 0xFF;
    }
    let span = if files != 0 {
        most_significant_bit(files) as i32 - least_significant_bit(files) as i32
    } else {
        0
    };
    add_eval(
        color,
        &mut pei.score,
        eval_pair(&tables.pawn_special, PAWN_FILE_SPAN) * span,
    );
    pe.draw[color] = (((7 - span) * (5 - population_count(files)).max(0)) as u8) as u8;
}

// Keep rustc from warning about the unused `free_area` shim.
#[allow(dead_code)]
fn _unused(eval_info: &EvalInfo) -> u64 {
    free_area(eval_info, WHITE)
}

#[cfg(test)]
mod tests {
    use crate::position::Engine;

    #[test]
    fn start_position_is_balanced() {
        let mut e = Engine::with_hash_mb(1);
        e.evaluate();
        assert_eq!(e.pos.current().score, 0);
    }

    #[test]
    fn evaluation_is_colour_symmetric() {
        let fens = [
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
            "2r3k1/pp3pp1/4p2p/3n4/3P4/2NB1P2/PP3P1P/2R3K1 w - - 0 1",
        ];
        for fen in fens {
            let mut e = Engine::with_hash_mb(1);
            e.set_fen(fen);
            e.evaluate();
            let white_score = e.pos.current().score;
            // Flip the board colours and side to move.
            let flipped = flip_fen(fen);
            let mut e2 = Engine::with_hash_mb(1);
            e2.set_fen(&flipped);
            e2.evaluate();
            assert_eq!(white_score, e2.pos.current().score, "{fen} vs {flipped}");
        }
    }

    fn flip_fen(fen: &str) -> String {
        let f: Vec<&str> = fen.split_whitespace().collect();
        let mut rows: Vec<String> = f[0]
            .split('/')
            .map(|r| {
                let mut s = String::new();
                for c in r.chars() {
                    if c.is_ascii_digit() {
                        s.push(c);
                    } else if c.is_ascii_uppercase() {
                        s.push(c.to_ascii_lowercase());
                    } else {
                        s.push(c.to_ascii_uppercase());
                    }
                }
                s
            })
            .collect();
        rows.reverse();
        let side = if f[1] == "w" { "b" } else { "w" };
        let castle: String = f[2]
            .chars()
            .map(|c| {
                if c.is_ascii_uppercase() {
                    c.to_ascii_lowercase()
                } else if c.is_ascii_lowercase() {
                    c.to_ascii_uppercase()
                } else {
                    c
                }
            })
            .collect();
        let ep = if f[3] == "-" {
            "-".to_string()
        } else {
            "-".to_string()
        };
        format!("{} {} {} {} 0 1", rows.join("/"), side, castle, ep)
    }
}

#[cfg(test)]
mod extra_tests {
    use super::*;
    use crate::position::Engine;

    fn eval(fen: &str) -> i32 {
        let mut e = Engine::with_hash_mb(1);
        e.set_fen(fen);
        e.evaluate();
        e.pos.current().score as i32
    }

    #[test]
    fn extra_queen_is_a_large_advantage() {
        let without = eval("4k3/8/8/8/8/8/8/4K3 w - - 0 1");
        let with = eval("4k3/8/8/8/8/8/8/3QK3 w - - 0 1");
        assert!(
            with - without > 700,
            "queen advantage too small: {with} vs {without}"
        );
    }

    #[test]
    fn extra_pawn_is_positive() {
        let without = eval("4k3/8/8/8/8/8/8/4K3 w - - 0 1");
        let with = eval("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1");
        assert!(with > without, "pawn advantage: {with} vs {without}");
    }

    #[test]
    fn central_knight_is_better_than_corner_knight() {
        let corner = eval("4k3/7p/8/8/8/8/P7/N3K3 w - - 0 1");
        let centre = eval("4k3/7p/8/8/4N3/8/P7/4K3 w - - 0 1");
        assert!(centre > corner, "knight PST: {centre} vs {corner}");
    }

    #[test]
    fn opening_and_endgame_packs_round_trip() {
        for open in [-300, -1, 0, 1, 300] {
            for end in [-300, -1, 0, 1, 300] {
                let packed = compose(open, end);
                assert_eq!(opening(packed), open);
                assert_eq!(endgame(packed), end);
            }
        }
    }
}
