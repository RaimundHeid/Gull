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
    pub pst: i32,
    pub material: i32,
    pub pawn_score: i32,
}

#[derive(Clone, Copy)]
struct EvalInfo {
    score: i32,
    king_w: usize,
    king_b: usize,
    mul: i32,
    occ: u64,
    area_w: u64,
    area_b: u64,
    free_w: u64,
    free_b: u64,
    king_att_w: u32,
    king_att_b: u32,
    pawn_index: usize,
    material: Material,
}

#[derive(Clone, Copy)]
struct PawnEvalInfo {
    king_w: usize,
    king_b: usize,
    score: i32,
    patt_w: u64,
    patt_b: u64,
    double_att_w: u64,
    double_att_b: u64,
}

#[inline(always)]
fn free_bb(ei: &EvalInfo, me: Color) -> u64 {
    if me == WHITE {
        ei.free_w
    } else {
        ei.free_b
    }
}

#[inline(always)]
fn area_bb(ei: &EvalInfo, me: Color) -> u64 {
    if me == WHITE {
        ei.area_w
    } else {
        ei.area_b
    }
}

#[inline(always)]
fn king_sq(ei: &EvalInfo, me: Color) -> usize {
    if me == WHITE {
        ei.king_w
    } else {
        ei.king_b
    }
}

#[inline(always)]
fn set_free(ei: &mut EvalInfo, me: Color, v: u64) {
    if me == WHITE {
        ei.free_w = v;
    } else {
        ei.free_b = v;
    }
}

#[inline(always)]
fn or_free(ei: &mut EvalInfo, me: Color, v: u64) {
    if me == WHITE {
        ei.free_w |= v;
    } else {
        ei.free_b |= v;
    }
}

#[inline(always)]
fn king_att(ei: &EvalInfo, me: Color) -> u32 {
    if me == WHITE {
        ei.king_att_w
    } else {
        ei.king_att_b
    }
}

#[inline(always)]
fn add_king_att(ei: &mut EvalInfo, me: Color, x: i32) {
    let v = x as u32;
    if me == WHITE {
        ei.king_att_w = ei.king_att_w.wrapping_add(v);
    } else {
        ei.king_att_b = ei.king_att_b.wrapping_add(v);
    }
}

#[inline(always)]
fn incv(me: Color, var: &mut i32, x: i32) {
    if me == WHITE {
        *var += x;
    } else {
        *var -= x;
    }
}

#[inline(always)]
fn decv(me: Color, var: &mut i32, x: i32) {
    if me == WHITE {
        *var -= x;
    } else {
        *var += x;
    }
}

#[inline(always)]
fn ca(arr: &[i32], y: usize) -> i32 {
    compose(arr[y * 2], arr[y * 2 + 1])
}

#[inline(always)]
fn pvar_free(ei: &EvalInfo, me: Color) -> u64 {
    free_bb(ei, me)
}

/// `evaluation()`.
pub fn evaluate(e: &mut Engine) {
    e.evaluate();
}

impl Engine {
    #[inline(always)]
    pub fn evaluate(&mut self) {
        let cur = self.pos.cur;
        let key = self.pos.data[cur].key;
        if self.pos.data[cur].eval_key == key {
            return;
        }
        self.pos.data[cur].eval_key = key;

        let t = self.tables;
        let occ = self.piece_all();
        let king_w = lsb(self.king(WHITE));
        let king_b = lsb(self.king(BLACK));
        let patt_w =
            Self::shift_w(WHITE, self.pawn(WHITE)) | Self::shift_e(WHITE, self.pawn(WHITE));
        let patt_b =
            Self::shift_w(BLACK, self.pawn(BLACK)) | Self::shift_e(BLACK, self.pawn(BLACK));
        let area_w = (t.s_area[king_w] | self.king(WHITE)) & ((!patt_w) | patt_b);
        let area_b = (t.s_area[king_b] | self.king(BLACK)) & ((!patt_b) | patt_w);
        self.pos.data[cur].patt[WHITE] = patt_w;
        self.pos.data[cur].patt[BLACK] = patt_b;
        self.pos.data[cur].att[WHITE] = patt_w;
        self.pos.data[cur].att[BLACK] = patt_b;
        self.pos.data[cur].passer = 0;
        self.pos.data[cur].threat =
            (patt_w & self.non_pawn(BLACK)) | (patt_b & self.non_pawn(WHITE));

        let mut ei = EvalInfo {
            score: self.pos.data[cur].pst,
            king_w,
            king_b,
            mul: 32,
            occ,
            area_w,
            area_b,
            free_w: 0,
            free_b: 0,
            king_att_w: 0,
            king_att_b: 0,
            pawn_index: 0,
            material: Material::default(),
        };

        // White.
        self.pos.data[cur].xray[WHITE] = 0;
        let free_w = self.queen(BLACK)
            | self.king(BLACK)
            | (!(patt_b | self.pawn(WHITE) | self.king(WHITE)));
        set_free(&mut ei, WHITE, free_w);
        let blocked = popcount(Self::shift(BLACK, ei.occ) & self.pawn(WHITE))
            * ca(&t.pawn_special, PAWN_BLOCKED);
        decv(WHITE, &mut ei.score, blocked);
        if (patt_w & ei.area_b) != 0 {
            ei.king_att_w = king_attack() as u32;
        }
        self.eval_queens(WHITE, &mut ei);
        or_free(&mut ei, WHITE, self.rook(BLACK));
        self.eval_rooks(WHITE, &mut ei);
        or_free(&mut ei, WHITE, self.minor(BLACK));
        self.eval_bishops(WHITE, &mut ei);
        self.eval_knights(WHITE, &mut ei);

        // Black.
        self.pos.data[cur].xray[BLACK] = 0;
        let free_b = self.queen(WHITE)
            | self.king(WHITE)
            | (!(patt_w | self.pawn(BLACK) | self.king(BLACK)));
        set_free(&mut ei, BLACK, free_b);
        let blocked = popcount(Self::shift(WHITE, ei.occ) & self.pawn(BLACK))
            * ca(&t.pawn_special, PAWN_BLOCKED);
        decv(BLACK, &mut ei.score, blocked);
        if (patt_b & ei.area_w) != 0 {
            ei.king_att_b = king_attack() as u32;
        }
        self.eval_queens(BLACK, &mut ei);
        or_free(&mut ei, BLACK, self.rook(WHITE));
        self.eval_rooks(BLACK, &mut ei);
        or_free(&mut ei, BLACK, self.minor(WHITE));
        self.eval_bishops(BLACK, &mut ei);
        self.eval_knights(BLACK, &mut ei);

        // Pawn hash / pawn structure.
        let pawn_key = self.pos.data[cur].pawn_key;
        let idx = (pawn_key & PAWN_HASH_MASK as u64) as usize;
        if (pawn_key >> 16) != self.pawn_hash[idx].key48() {
            eval_pawn_structure(&self.pos, &self.tables, &mut self.pawn_hash[idx]);
        }
        ei.pawn_index = idx;
        ei.score += self.pawn_hash[idx].score;
        let pawn_score = self.pawn_hash[idx].score;

        self.eval_king(WHITE, &mut ei);
        self.eval_king(BLACK, &mut ei);
        self.pos.data[cur].att[WHITE] |= t.s_area[ei.king_w];
        self.pos.data[cur].att[BLACK] |= t.s_area[ei.king_b];

        self.eval_passer(WHITE, &mut ei);
        self.eval_pieces(WHITE, &mut ei);
        self.eval_passer(BLACK, &mut ei);
        self.eval_pieces(BLACK, &mut ei);

        if (self.pos.data[cur].material & FLAG_UNUSUAL_MATERIAL) != 0 {
            self.eval_unusual_material(&ei);
            return;
        }
        let material = t.material[self.pos.data[cur].material as usize];
        ei.material = material;
        let phase = material.phase as i32;
        let base = material.score as i32
            + ((opening(ei.score) * phase) + (endgame(ei.score) * (128 - phase))) / 128;
        self.pos.data[cur].score = base as i16;

        if self.pos.data[cur].ply >= 50 {
            self.pos.data[cur].score /= 2;
        }
        let s = self.pos.data[cur].score as i32;
        if s > 0 {
            ei.mul = material.mul[WHITE] as i32;
            if (material.flags & FLAG_CALL_EVAL_ENDGAME_W) != 0 {
                self.eval_endgame(WHITE, &mut ei);
            }
            let cs = self.pos.data[cur].score as i32;
            let draw = self.pawn_hash[ei.pawn_index].draw[WHITE] as i32;
            self.pos.data[cur].score = (cs - (cs.min(100) * draw) / 64) as i16;
        } else if s < 0 {
            ei.mul = material.mul[BLACK] as i32;
            if (material.flags & FLAG_CALL_EVAL_ENDGAME_B) != 0 {
                self.eval_endgame(BLACK, &mut ei);
            }
            let cs = self.pos.data[cur].score as i32;
            let draw = self.pawn_hash[ei.pawn_index].draw[BLACK] as i32;
            self.pos.data[cur].score = (cs + ((-cs).min(100) * draw) / 64) as i16;
        } else {
            ei.mul = (material.mul[WHITE] as i32).min(material.mul[BLACK] as i32);
        }
        let cs = self.pos.data[cur].score as i32;
        self.pos.data[cur].score = ((cs * ei.mul) / 32) as i16;

        if self.pos.data[cur].turn != 0 {
            self.pos.data[cur].score = (-(self.pos.data[cur].score as i32)) as i16;
        }
        self.update_delta();

        let _ = pawn_score;
    }

    /// Evaluate and return a trace of the main components (used by tests).
    pub fn evaluate_trace(&mut self) -> EvalTrace {
        self.evaluate();
        let st = &self.pos.data[self.pos.cur];
        let idx = (st.pawn_key & PAWN_HASH_MASK as u64) as usize;
        let _ = idx;
        EvalTrace {
            score: st.score as i32,
            pst: st.pst,
            material: st.material,
            pawn_score: 0,
        }
    }

    /// `UpdateDelta` macro.
    pub fn update_delta(&mut self) {
        let cur = self.pos.cur;
        let st = self.pos.data[cur];
        if st.capture != 0 || st.mv == 0 || (st.mv as i32 & 0xE000) != 0 || cur == 0 {
            return;
        }
        let mv = st.mv as i32;
        let piece = st.piece as usize;
        let from = move_from(mv);
        let to = move_to(mv);
        let idx = (piece << 12) | (from << 6) | to;
        let prev = self.pos.data[cur - 1].score as i32;
        let target = -(st.score as i32) - prev;
        if (self.delta[idx] as i32) <= target {
            self.delta[idx] = target as i16;
        } else {
            self.delta[idx] = self.delta[idx].wrapping_sub(1);
        }
    }

    // -- piece evaluation --------------------------------------------------

    #[inline(always)]
    fn eval_queens(&mut self, me: Color, ei: &mut EvalInfo) {
        let t = self.tables;
        let cur = self.pos.cur;
        let oppc = opp(me);
        let mut u = self.queen(me);
        while u != 0 {
            let sq = lsb(u);
            let b = bit(sq);
            u ^= b;
            let att = t.queen_attacks(sq, ei.occ);
            self.pos.data[cur].att[me] |= att;
            if (t.q_mask[sq] & self.king(oppc)) != 0 {
                let mut v = t.between[king_sq(ei, oppc)][sq] & ei.occ;
                if v != 0 {
                    if single(v) {
                        self.pos.data[cur].xray[me] |= v;
                        let square = lsb(v);
                        let piece = self.pos.board.square[square] as usize;
                        let mut katt = false;
                        if piece == ipawn(me) {
                            if self.pos.board.square[(square as i32 + Self::push(me)) as usize] == 0
                            {
                                incv(me, &mut ei.score, ca(&t.pin, SELF_PAWN_PIN));
                            }
                        } else if (piece & 1) == me {
                            incv(me, &mut ei.score, ca(&t.pin, SELF_PIECE_PIN));
                            katt = true;
                        } else if piece != ipawn(oppc)
                            && ((((t.b_mask[sq] & self.bishop(oppc))
                                | (t.r_mask[sq] & self.rook(oppc))
                                | self.queen(oppc))
                                & v)
                                == 0)
                        {
                            incv(me, &mut ei.score, ca(&t.pin, WEAK_PIN));
                            if (self.pos.data[cur].patt[oppc] & v) == 0 {
                                katt = true;
                            }
                        }
                        if katt && (att & area_bb(ei, oppc)) == 0 {
                            add_king_att(ei, me, king_attack());
                        }
                    } else {
                        if v == (v & self.minor(oppc)) {
                            incv(me, &mut ei.score, ca(&t.king_ray, Q_KING_RAY));
                        }
                    }
                }
            }
            if (att & area_bb(ei, oppc)) != 0 {
                add_king_att(ei, me, king_q_attack());
                let mut v = att & area_bb(ei, oppc);
                while v != 0 {
                    let s = lsb(v);
                    cut(&mut v);
                    if (t.full_line[sq][s]
                        & att
                        & ((self.rook(me) & t.r_mask[sq]) | (self.bishop(me) & t.b_mask[sq])))
                        != 0
                    {
                        add_king_att(ei, me, 1);
                    }
                }
            }
            let free = pvar_free(ei, me);
            let mob = popcount(att & free) as usize;
            let mob_score = t.mobility[PIECE_TYPE[WHITE_QUEEN] - 1][mob];
            incv(me, &mut ei.score, mob_score);
            if (att & free & self.pawn(oppc)) != 0 {
                incv(me, &mut ei.score, ca(&t.tactical, TACTICAL_MAJOR_PAWN));
            }
            if (att & free & self.minor(oppc)) != 0 {
                incv(me, &mut ei.score, ca(&t.tactical, TACTICAL_MAJOR_MINOR));
            }
            if (att & area_bb(ei, me)) != 0 {
                incv(me, &mut ei.score, ca(&t.king_defence, KING_DEF_QUEEN));
            }
        }
    }

    #[inline(always)]
    fn eval_rooks(&mut self, me: Color, ei: &mut EvalInfo) {
        let t = self.tables;
        let cur = self.pos.cur;
        let oppc = opp(me);
        let mut u = self.rook(me);
        while u != 0 {
            let sq = lsb(u);
            let b = bit(sq);
            u ^= b;
            let att = t.rook_attacks(sq, ei.occ);
            self.pos.data[cur].att[me] |= att;
            if (t.r_mask[sq] & self.king(oppc)) != 0 {
                let mut v = t.between[king_sq(ei, oppc)][sq] & ei.occ;
                if v != 0 {
                    if single(v) {
                        self.pos.data[cur].xray[me] |= v;
                        let square = lsb(v);
                        let piece = self.pos.board.square[square] as usize;
                        let mut katt = false;
                        if piece == ipawn(me) {
                            if self.pos.board.square[(square as i32 + Self::push(me)) as usize] == 0
                            {
                                incv(me, &mut ei.score, ca(&t.pin, SELF_PAWN_PIN));
                            }
                        } else if (piece & 1) == me {
                            incv(me, &mut ei.score, ca(&t.pin, SELF_PIECE_PIN));
                            katt = true;
                        } else if piece != ipawn(oppc) {
                            if piece < irook(oppc) {
                                incv(me, &mut ei.score, ca(&t.pin, WEAK_PIN));
                                if (self.pos.data[cur].patt[oppc] & v) == 0 {
                                    katt = true;
                                }
                            } else if piece == iqueen(oppc) {
                                incv(me, &mut ei.score, ca(&t.pin, THREAT_PIN));
                            }
                        }
                        if katt && (att & area_bb(ei, oppc)) == 0 {
                            add_king_att(ei, me, king_attack());
                        }
                    } else if v == (v & (self.minor(oppc) | self.queen(oppc))) {
                        incv(me, &mut ei.score, ca(&t.king_ray, R_KING_RAY));
                    }
                }
            }
            if (att & area_bb(ei, oppc)) != 0 {
                add_king_att(ei, me, king_r_attack());
                let mut v = att & area_bb(ei, oppc);
                while v != 0 {
                    let s = lsb(v);
                    cut(&mut v);
                    if (t.full_line[sq][s] & att & self.major(me)) != 0 {
                        add_king_att(ei, me, 1);
                    }
                }
            }
            let free = pvar_free(ei, me);
            let mob = popcount(att & free) as usize;
            let mob_score = t.mobility[PIECE_TYPE[WHITE_ROOK] - 1][mob];
            incv(me, &mut ei.score, mob_score);
            if (att & free & self.pawn(oppc)) != 0 {
                incv(me, &mut ei.score, ca(&t.tactical, TACTICAL_MAJOR_PAWN));
            }
            if (att & free & self.minor(oppc)) != 0 {
                incv(me, &mut ei.score, ca(&t.tactical, TACTICAL_MAJOR_MINOR));
            }
            if (att & area_bb(ei, me)) != 0 {
                incv(me, &mut ei.score, ca(&t.king_defence, KING_DEF_ROOK));
            }
            self.pos.data[cur].threat |= att & self.queen(oppc);
            if (t.p_way[me][sq] & self.pawn(me)) == 0 {
                incv(me, &mut ei.score, ca(&t.rook_special, ROOK_HOF));
                let mut hof_score = 0i32;
                if (t.p_way[me][sq] & self.pawn(oppc)) == 0 {
                    incv(me, &mut ei.score, ca(&t.rook_special, ROOK_OF));
                    if (att & self.line(me, 7)) != 0 {
                        hof_score += ca(&t.rook_special, ROOK_OF_OPEN);
                    } else {
                        let target = att & t.p_way[me][sq] & self.minor(oppc);
                        if target != 0 {
                            if (self.pos.data[cur].patt[oppc] & target) == 0 {
                                hof_score += ca(&t.rook_special, ROOK_OF_MINOR_HAGING);
                                if (t.p_way[me][sq] & self.king(oppc)) != 0 {
                                    hof_score += ca(&t.rook_special, ROOK_OF_KING_ATT);
                                }
                            } else {
                                hof_score += ca(&t.rook_special, ROOK_OF_MINOR_FIXED);
                            }
                        }
                    }
                } else if (att & t.p_way[me][sq] & self.pawn(oppc)) != 0 {
                    let square = lsb(att & t.p_way[me][sq] & self.pawn(oppc));
                    if (t.p_support[oppc][square] & self.pawn(oppc)) == 0 {
                        hof_score += ca(&t.rook_special, ROOK_HOF_WEAK_P_ATT);
                    }
                }
                incv(me, &mut ei.score, hof_score);
                if (t.p_way[oppc][sq] & att & self.major(me)) != 0 {
                    incv(me, &mut ei.score, hof_score);
                }
            }
            if (b & self.line(me, 6)) != 0
                && ((self.king(oppc) | self.pawn(oppc)) & (self.line(me, 6) | self.line(me, 7)))
                    != 0
            {
                incv(me, &mut ei.score, ca(&t.rook_special, ROOK_7TH));
                if (self.king(oppc) & self.line(me, 7)) != 0 {
                    incv(me, &mut ei.score, ca(&t.rook_special, ROOK_7TH_K8TH));
                }
                if (self.major(me) & att & self.line(me, 6)) != 0 {
                    incv(me, &mut ei.score, ca(&t.rook_special, ROOK_7TH_DOUBLED));
                }
            }
        }
    }

    #[inline(always)]
    fn eval_bishops(&mut self, me: Color, ei: &mut EvalInfo) {
        let t = self.tables;
        let cur = self.pos.cur;
        let oppc = opp(me);
        let mut u = self.bishop(me);
        while u != 0 {
            let sq = lsb(u);
            let b = bit(sq);
            u ^= b;
            let att = t.bishop_attacks(sq, ei.occ);
            self.pos.data[cur].att[me] |= att;
            if (t.b_mask[sq] & self.king(oppc)) != 0 {
                let mut v = t.between[king_sq(ei, oppc)][sq] & ei.occ;
                if v != 0 {
                    if single(v) {
                        self.pos.data[cur].xray[me] |= v;
                        let square = lsb(v);
                        let piece = self.pos.board.square[square] as usize;
                        let mut katt = false;
                        if piece == ipawn(me) {
                            if self.pos.board.square[(square as i32 + Self::push(me)) as usize] == 0
                            {
                                incv(me, &mut ei.score, ca(&t.pin, SELF_PAWN_PIN));
                            }
                        } else if (piece & 1) == me {
                            incv(me, &mut ei.score, ca(&t.pin, SELF_PIECE_PIN));
                            katt = true;
                        } else if piece != ipawn(oppc) {
                            if piece < ilight(oppc) {
                                incv(me, &mut ei.score, ca(&t.pin, STRONG_PIN));
                                if (self.pos.data[cur].patt[oppc] & v) == 0 {
                                    katt = true;
                                }
                            } else if piece >= irook(oppc) {
                                incv(me, &mut ei.score, ca(&t.pin, THREAT_PIN));
                            }
                        }
                        if katt && (att & area_bb(ei, oppc)) == 0 {
                            add_king_att(ei, me, king_attack());
                        }
                    } else if v == (v & (self.knight(oppc) | self.major(oppc))) {
                        incv(me, &mut ei.score, ca(&t.king_ray, B_KING_RAY));
                    }
                }
            }
            if (att & area_bb(ei, oppc)) != 0 {
                add_king_att(ei, me, king_b_attack());
            }
            let free = pvar_free(ei, me);
            let mob = popcount(att & free) as usize;
            let mob_score = t.mobility[PIECE_TYPE[WHITE_LIGHT] - 1][mob];
            incv(me, &mut ei.score, mob_score);
            if (att & free & self.pawn(oppc)) != 0 {
                incv(me, &mut ei.score, ca(&t.tactical, TACTICAL_MINOR_PAWN));
            }
            if (att & free & self.knight(oppc)) != 0 {
                incv(me, &mut ei.score, ca(&t.tactical, TACTICAL_MINOR_MINOR));
            }
            if (att & area_bb(ei, me)) != 0 {
                incv(me, &mut ei.score, ca(&t.king_defence, KING_DEF_BISHOP));
            }
            self.pos.data[cur].threat |= att & self.major(oppc);
            let (light_area, other_area) = if (b & LIGHT_AREA) != 0 {
                (LIGHT_AREA, DARK_AREA)
            } else {
                (DARK_AREA, LIGHT_AREA)
            };
            let _ = other_area;
            let mut v = ((!t.bishop_forward[me][sq]) | (att & t.forward[me][rank(sq)]))
                & self.pawn(oppc)
                & (!self.pos.data[cur].patt[oppc])
                & light_area;
            while v != 0 {
                let square = lsb(v);
                cut(&mut v);
                if ((t.p_support[oppc][square] | t.p_way[oppc][square]) & self.pawn(oppc)) == 0 {
                    incv(
                        me,
                        &mut ei.score,
                        ca(&t.bishop_special, BISHOP_NON_FORWARD_PAWN),
                    );
                }
            }
            let mut v = t.bishop_forward[me][sq] & self.pawn(me) & light_area;
            v |= (v & (t.file[2] | t.file[3] | t.file[4] | t.file[5] | t.b_mask[sq])) >> 8;
            decv(
                me,
                &mut ei.score,
                ca(&t.bishop_special, BISHOP_PAWN_BLOCK) * popcount(v),
            );
        }
    }

    #[inline(always)]
    fn eval_knights(&mut self, me: Color, ei: &mut EvalInfo) {
        let t = self.tables;
        let cur = self.pos.cur;
        let oppc = opp(me);
        let mut u = self.knight(me);
        while u != 0 {
            let sq = lsb(u);
            let b = bit(sq);
            u ^= b;
            let att = t.n_att[sq];
            self.pos.data[cur].att[me] |= att;
            if (att & area_bb(ei, oppc)) != 0 {
                add_king_att(ei, me, king_n_attack());
            }
            let free = pvar_free(ei, me);
            let mob = popcount(att & free) as usize;
            let mob_score = t.mobility[PIECE_TYPE[WHITE_KNIGHT] - 1][mob];
            incv(me, &mut ei.score, mob_score);
            if (att & free & self.pawn(oppc)) != 0 {
                incv(me, &mut ei.score, ca(&t.tactical, TACTICAL_MINOR_PAWN));
            }
            if (att & free & self.bishop(oppc)) != 0 {
                incv(me, &mut ei.score, ca(&t.tactical, TACTICAL_MINOR_MINOR));
            }
            if (att & area_bb(ei, me)) != 0 {
                incv(me, &mut ei.score, ca(&t.king_defence, KING_DEF_KNIGHT));
            }
            self.pos.data[cur].threat |= att & self.major(oppc);
            if (b & OUTPOST[me]) != 0
                && (self.pawn(oppc) & t.p_isolated[file(sq)] & t.forward[me][rank(sq)]) == 0
            {
                incv(me, &mut ei.score, ca(&t.knight_special, KNIGHT_OUTPOST));
                if (self.pos.data[cur].patt[me] & b) != 0 {
                    incv(
                        me,
                        &mut ei.score,
                        ca(&t.knight_special, KNIGHT_OUTPOST_PROTECTED),
                    );
                    if (att & pvar_free(ei, me) & self.pawn(oppc)) != 0 {
                        incv(
                            me,
                            &mut ei.score,
                            ca(&t.knight_special, KNIGHT_OUTPOST_PAWN_ATT),
                        );
                    }
                    if (att & free & self.bishop(oppc)) != 0 {
                        incv(
                            me,
                            &mut ei.score,
                            ca(&t.knight_special, KNIGHT_OUTPOST_BISHOP_ATT),
                        );
                    }
                }
            }
        }
    }

    #[inline(always)]
    fn eval_king(&mut self, me: Color, ei: &mut EvalInfo) {
        let t = self.tables;
        let oppc = opp(me);
        let cur = self.pos.cur;
        let cnt = 15.min(opening(king_att(ei, me) as i32));
        let mut score = endgame(king_att(ei, me) as i32);
        let shelter_opp = self.pawn_hash[ei.pawn_index].shelter[oppc] as i32;
        if cnt >= 2 && self.queen(me) != 0 {
            score += (shelter_opp * king_shelter_quad()) / 64;
            let u =
                self.pos.data[cur].att[me] & area_bb(ei, oppc) & (!self.pos.data[cur].att[oppc]);
            if u != 0 {
                score += popcount(u) * king_attack_square();
            }
            if (t.s_area[king_sq(ei, oppc)] & (!(self.piece(oppc) | self.pos.data[cur].att[me])))
                == 0
            {
                score += king_no_moves();
            }
        }
        let mut adjusted = ((score * t.king_attack_scale[cnt as usize]) >> 3) + shelter_opp;
        if self.queen(me) == 0 {
            adjusted /= 2;
        }
        incv(me, &mut ei.score, adjusted);
    }

    #[inline(always)]
    fn eval_passer(&mut self, me: Color, ei: &mut EvalInfo) {
        let t = self.tables;
        let cur = self.pos.cur;
        let oppc = opp(me);
        let mut u = self.pawn_hash[ei.pawn_index].passer[me] as u64;
        while u != 0 {
            let f = lsb(u);
            cut(&mut u);
            let passer = t.file[f] & self.pawn(me);
            if passer == 0 {
                continue;
            }
            let sq = nb(oppc, passer);
            let r = crank(me, sq);
            self.pos.data[cur].passer |= bit(sq);
            if r <= 2 {
                continue;
            }
            if self.pos.board.square[(sq as i32 + Self::push(me)) as usize] == 0 {
                incv(me, &mut ei.score, t.passer_blocked[r]);
            }
            let way = t.p_way[me][sq];
            let mut connected = 0;
            let mut supported = false;
            let mut hooked = false;
            let mut free = false;
            if (way & self.piece(oppc)) == 0 {
                incv(me, &mut ei.score, t.passer_clear[r]);
                if (t.p_way[oppc][sq] & self.major(me)) != 0 {
                    let square = nb(oppc, t.p_way[oppc][sq] & self.major(me));
                    if (t.between[sq][square] & ei.occ) == 0 {
                        supported = true;
                    }
                }
                if (t.p_way[oppc][sq] & self.major(oppc)) != 0 {
                    let square = nb(oppc, t.p_way[oppc][sq] & self.major(oppc));
                    if (t.between[sq][square] & ei.occ) == 0 {
                        hooked = true;
                    }
                }
                let mut v = t.p_att[me][(sq as i32 - Self::push(me)) as usize] & self.pawn(me);
                while v != 0 {
                    let square = lsb(v);
                    cut(&mut v);
                    if (self.pawn(oppc)
                        & (t.file[file(square)] | t.p_isolated[file(square)])
                        & t.forward[me][rank(square)])
                        == 0
                    {
                        connected += 1;
                    }
                }
                if connected != 0 {
                    incv(me, &mut ei.score, t.passer_connected[r]);
                }
                if !hooked && (self.pos.data[cur].att[oppc] & way) == 0 {
                    incv(me, &mut ei.score, t.passer_free[r]);
                    free = true;
                } else {
                    let attacked = self.pos.data[cur].att[oppc] | if hooked { way } else { 0 };
                    if supported
                        || (!hooked && connected != 0)
                        || ((self.major(me) & way) == 0
                            && (attacked & (!self.pos.data[cur].att[me])) == 0)
                    {
                        incv(me, &mut ei.score, t.passer_supported[r]);
                    }
                }
            }
            if r == 6 {
                if (way & self.rook(me)) != 0
                    && self.minor(me) == 0
                    && self.queen(me) == 0
                    && single(self.rook(me))
                {
                    decv(
                        me,
                        &mut ei.score,
                        compose(0, t.passer_special[PASSER_OP_ROOK_BLOCK]),
                    );
                }
                if self.major(oppc) == 0
                    && (self.non_pawn_king(oppc) == 0 || single(self.non_pawn_king(oppc)))
                {
                    incv(
                        me,
                        &mut ei.score,
                        compose(0, t.passer_special[PASSER_ONE_PIECE]),
                    );
                    if !free {
                        if (t.s_area[(sq as i32 + Self::push(me)) as usize] & self.king(oppc)) == 0
                        {
                            incv(
                                me,
                                &mut ei.score,
                                compose(0, t.passer_special[PASSER_OP_MINOR_CONTROL]),
                            );
                        } else {
                            incv(
                                me,
                                &mut ei.score,
                                compose(0, t.passer_special[PASSER_OP_KING_CONTROL]),
                            );
                        }
                    }
                }
            }
        }
    }

    #[inline(always)]
    fn eval_pieces(&mut self, me: Color, ei: &mut EvalInfo) {
        let t = self.tables;
        let cur = self.pos.cur;
        let oppc = opp(me);
        self.pos.data[cur].threat |=
            self.pos.data[cur].att[oppc] & (!self.pos.data[cur].att[me]) & self.piece(me);
        let mut u = self.pos.data[cur].threat & self.piece(me);
        if u != 0 {
            decv(me, &mut ei.score, ca(&t.tactical, TACTICAL_THREAT));
            u &= u - 1;
            if u != 0 {
                decv(
                    me,
                    &mut ei.score,
                    ca(&t.tactical, TACTICAL_THREAT) + ca(&t.tactical, TACTICAL_DOUBLE_THREAT),
                );
                u &= u - 1;
                while u != 0 {
                    decv(me, &mut ei.score, ca(&t.tactical, TACTICAL_THREAT));
                    u &= u - 1;
                }
            }
        }
    }

    fn eval_unusual_material(&mut self, ei: &EvalInfo) {
        let t = self.tables;
        let cur = self.pos.cur;
        let wp = popcount(self.pawn(WHITE));
        let bp = popcount(self.pawn(BLACK));
        let wlight = popcount(self.minor(WHITE));
        let blight = popcount(self.minor(BLACK));
        let wr = popcount(self.rook(WHITE));
        let br = popcount(self.rook(BLACK));
        let wq = popcount(self.queen(WHITE));
        let bq = popcount(self.queen(BLACK));
        let phase = 24.min((wlight + blight) + 2 * (wr + br) + 4 * (wq + bq));
        let mat_score = SEE_VALUE[WHITE_PAWN] * (wp - bp)
            + SEE_VALUE[WHITE_KNIGHT] * (wlight - blight)
            + SEE_VALUE[WHITE_ROOK] * (wr - br)
            + SEE_VALUE[WHITE_QUEEN] * (wq - bq);
        let mat_score = compose(mat_score, mat_score);
        self.pos.data[cur].score = (((opening(mat_score + ei.score) * phase)
            + (endgame(mat_score + ei.score) * (24 - phase)))
            / 24) as i16;
        if self.pos.data[cur].turn != 0 {
            self.pos.data[cur].score = (-(self.pos.data[cur].score as i32)) as i16;
        }
        let _ = t;
        self.update_delta();
    }

    // -- endgame scaling ---------------------------------------------------

    #[inline(always)]
    fn eval_endgame(&mut self, me: Color, ei: &mut EvalInfo) {
        let t = self.tables;
        let cur = self.pos.cur;
        let oppc = opp(me);
        let material = ei.material;
        let flag = if me == WHITE {
            FLAG_SINGLE_BISHOP_W
        } else {
            FLAG_SINGLE_BISHOP_B
        };
        if (material.flags & flag) != 0 && self.pawn(me) != 0 {
            let sq = if self.pos.board.bb[ilight(me)] != 0 {
                if me == WHITE {
                    63
                } else {
                    0
                }
            } else if self.pos.board.bb[idark(me)] != 0 {
                if me == WHITE {
                    56
                } else {
                    7
                }
            } else if file(lsb(self.king(oppc))) <= 3 {
                if me == WHITE {
                    56
                } else {
                    0
                }
            } else if me == WHITE {
                63
            } else {
                7
            };
            if (self.pawn(me) & (!t.p_way[oppc][sq])) == 0 {
                if ((t.s_area[sq] | bit(sq)) & self.king(oppc)) != 0 {
                    ei.mul = 0;
                } else if (t.s_area[sq] & t.s_area[lsb(self.king(oppc))] & self.line(me, 7)) != 0
                    && self.pos.board.square[(sq as i32 - Self::push(me)) as usize]
                        == ipawn(oppc) as u8
                    && self.pos.board.square[(sq as i32 - 2 * Self::push(me)) as usize]
                        == ipawn(me) as u8
                {
                    ei.mul = 0;
                }
            } else if ((self.king(oppc) & self.line(me, 6)) | self.line(me, 7)) != 0
                && (file(sq) as i32 - file(lsb(self.king(oppc))) as i32).abs() <= 3
                && (self.pawn(me) & (!t.p_support[me][sq])) == 0
                && (self.pawn(me) & self.line(me, 5) & Self::shift(oppc, self.pawn(oppc))) != 0
            {
                ei.mul = 0;
            }
            if single(self.pawn(me)) {
                if self.bishop(me) == 0 {
                    ei.mul = ei.mul.min(self.kpkx(me));
                    if self.piece(oppc) == self.king(oppc) && ei.mul == 32 {
                        if me == WHITE {
                            self.pos.data[cur].score =
                                (self.pos.data[cur].score as i32 + KPK_VALUE) as i16;
                        } else {
                            self.pos.data[cur].score =
                                (self.pos.data[cur].score as i32 - KPK_VALUE) as i16;
                        }
                    }
                } else {
                    let sq = lsb(self.pawn(me));
                    if (self.pawn(me) & (t.file[1] | t.file[6]) & self.line(me, 5)) != 0
                        && self.pos.board.square[(sq as i32 + Self::push(me)) as usize]
                            == ipawn(oppc) as u8
                        && ((t.p_att[me][(sq as i32 + Self::push(me)) as usize]
                            | t.p_way[me][(sq as i32 + Self::push(me)) as usize])
                            & self.king(oppc))
                            != 0
                    {
                        ei.mul = 0;
                    }
                }
            }
            if self.bishop(oppc) != 0
                && single(self.bishop(oppc))
                && ((self.pos.board.bb[ilight(me)] != 0) != (self.pos.board.bb[ilight(oppc)] != 0))
            {
                let mut pcnt = 0i32;
                let king_opp = king_sq(ei, oppc);
                let mut goto_partial = false;
                if ((self.king(oppc) & LIGHT_AREA) != 0) == ((self.bishop(oppc) & LIGHT_AREA) != 0)
                {
                    let mut u = self.pawn(me);
                    while u != 0 {
                        if pcnt >= 2 {
                            goto_partial = true;
                            break;
                        }
                        pcnt += 1;
                        let sq = lsb(u);
                        cut(&mut u);
                        if (t.p_way[me][sq] & (t.p_att[me][king_opp] | t.p_att[oppc][king_opp]))
                            == 0
                        {
                            if (t.p_way[me][sq] & self.pawn(oppc)) == 0 {
                                goto_partial = true;
                                break;
                            }
                            let bsq = lsb(self.bishop(oppc));
                            let att = t.bishop_attacks(bsq, ei.occ);
                            if (att & t.p_way[me][sq] & self.pawn(oppc)) == 0 {
                                goto_partial = true;
                                break;
                            }
                            if (t.bishop_forward[me][bsq] & att & t.p_way[me][sq] & self.pawn(oppc))
                                == 0
                                && popcount(
                                    t.full_line[lsb(att & t.p_way[me][sq] & self.pawn(oppc))][bsq]
                                        & att,
                                ) <= 2
                            {
                                goto_partial = true;
                                break;
                            }
                        }
                    }
                    if !goto_partial {
                        ei.mul = 0;
                        return;
                    }
                }
                if pcnt <= 2
                    && multiple(self.pawn(me))
                    && self.pawn(oppc) == 0
                    && (self.pawn(me) & BOUNDARY) == 0
                    && ei.mul != 0
                {
                    let sq1 = lsb(self.pawn(me));
                    let sq2 = msb(self.pawn(me));
                    let fd = (file(sq2) as i32 - file(sq1) as i32).abs();
                    if fd >= 5 {
                        ei.mul = 32;
                    } else if fd >= 4 {
                        ei.mul = 26;
                    } else if fd >= 3 {
                        ei.mul = 20;
                    }
                }
                if ((t.s_area[king_opp] | self.pos.data[cur].patt[oppc]) & self.bishop(oppc)) != 0 {
                    let push = Self::shift(me, self.pawn(me));
                    if (push & (!(self.piece(oppc) | self.pos.data[cur].att[oppc]))) == 0
                        && (self.king(oppc)
                            & if self.pos.board.bb[ilight(oppc)] != 0 {
                                LIGHT_AREA
                            } else {
                                DARK_AREA
                            })
                            != 0
                    {
                        ei.mul = ei.mul.min(8);
                        let bsq = lsb(self.bishop(oppc));
                        let att = t.bishop_attacks(bsq, ei.occ);
                        let prp = (att | t.s_area[king_opp])
                            & self.pawn(oppc)
                            & if self.pos.board.bb[ilight(oppc)] != 0 {
                                LIGHT_AREA
                            } else {
                                DARK_AREA
                            };
                        let patt = Self::shift_w(oppc, prp) | Self::shift_e(oppc, prp);
                        if ((t.s_area[king_opp] | patt) & self.bishop(oppc)) != 0 {
                            let double_att = (t.s_area[king_opp] & patt)
                                | (patt & att)
                                | (t.s_area[king_opp] & att);
                            if (push & (!(self.king(oppc) | self.bishop(oppc) | prp | double_att)))
                                == 0
                            {
                                ei.mul = 0;
                                return;
                            }
                        }
                    }
                }
            }
        }
        if self.major(me) == 0 {
            if self.bishop(me) != 0
                && self.knight(me) == 0
                && single(self.bishop(me))
                && self.pawn(me) != 0
            {
                let number = popcount(self.pawn(me));
                if number == 1 {
                    if self.bishop(oppc) != 0 {
                        ei.mul = ei.mul.min(self.kbpkbx(me));
                    } else if self.knight(oppc) != 0 {
                        ei.mul = ei.mul.min(self.kbpknx(me));
                    }
                } else if number == 2 && self.bishop(oppc) != 0 {
                    ei.mul = ei.mul.min(self.kbppkbx(me));
                }
            } else if self.bishop(me) == 0
                && self.knight(me) != 0
                && single(self.knight(me))
                && self.pawn(me) != 0
                && single(self.pawn(me))
            {
                ei.mul = ei.mul.min(self.knpkx(me));
            }
        } else if self.minor(me) == 0 {
            if self.pawn(me) == 0
                && self.rook(me) == 0
                && self.queen(me) != 0
                && self.pawn(oppc) != 0
            {
                if self.non_pawn_king(oppc) == 0 && single(self.pawn(oppc)) {
                    ei.mul = ei.mul.min(self.kqkp(me));
                } else if self.rook(oppc) != 0 {
                    ei.mul = ei.mul.min(self.kqkrpx(me));
                }
            } else if self.queen(me) == 0 && self.rook(me) != 0 && single(self.rook(me)) {
                let number = popcount(self.pawn(me));
                if number <= 3 {
                    if number == 0 {
                        if self.pawn(oppc) != 0 {
                            ei.mul = ei.mul.min(self.krkpx(me));
                        }
                    } else if self.rook(oppc) != 0 {
                        if number == 1 {
                            let new_mul = self.krpkrx(me);
                            ei.mul = if new_mul <= 32 {
                                ei.mul.min(new_mul)
                            } else {
                                new_mul
                            };
                        } else {
                            if number == 2 {
                                ei.mul = ei.mul.min(self.krppkrx(me));
                            }
                            if self.pawn(oppc) != 0 {
                                if number == 2 {
                                    ei.mul = ei.mul.min(self.krppkrpx(me));
                                } else if multiple(self.pawn(oppc)) {
                                    ei.mul = ei.mul.min(self.krpppkrppx(me));
                                }
                            }
                        }
                    } else if number == 1 && self.bishop(oppc) != 0 {
                        ei.mul = ei.mul.min(self.krpkbx(me));
                    }
                }
            }
        } else if self.pawn(me) == 0
            && single(self.rook(me))
            && self.queen(me) == 0
            && single(self.bishop(me))
            && self.knight(me) == 0
            && self.rook(oppc) != 0
        {
            ei.mul = ei.mul.min(self.krbkrx(me));
        }
        if self.non_pawn_king(oppc) == 0
            && self.pos.data[cur].turn == oppc as u8
            && (self.pos.data[cur].att[me] & self.king(oppc)) == 0
            && (t.s_area[king_sq(ei, oppc)] & (!(self.pos.data[cur].att[me] | self.piece(oppc))))
                == 0
            && (self.pos.data[cur].patt[oppc] & self.piece(me)) == 0
            && (Self::shift(oppc, self.pawn(oppc)) & (!ei.occ)) == 0
        {
            ei.mul = 0;
        }
    }

    fn kpkx(&self, me: Color) -> i32 {
        let t = self.tables;
        let turn = self.pos.data[self.pos.cur].turn as usize;
        let u = if me == WHITE {
            t.kpk(turn, lsb(self.pawn(WHITE)), lsb(self.king(WHITE))) & bit(lsb(self.king(BLACK)))
        } else {
            t.kpk(
                turn ^ 1,
                63 - lsb(self.pawn(BLACK)),
                63 - lsb(self.king(BLACK)),
            ) & bit(63 - lsb(self.king(WHITE)))
        };
        if u != 0 {
            32
        } else if (self.piece(opp(me)) ^ self.king(opp(me))) != 0 {
            1
        } else {
            0
        }
    }

    fn knpkx(&self, me: Color) -> i32 {
        let t = self.tables;
        if (self.pawn(me) & self.line(me, 6) & (t.file[0] | t.file[7])) != 0 {
            let sq = lsb(self.pawn(me));
            if (t.s_area[sq] & self.king(opp(me)) & (self.line(me, 6) | self.line(me, 7))) != 0 {
                return 0;
            }
            let p = (sq as i32 + Self::push(me)) as usize;
            if self.pos.board.square[p] == iking(me) as u8
                && (t.s_area[lsb(self.king(me))]
                    & t.s_area[lsb(self.king(opp(me)))]
                    & self.line(me, 7))
                    != 0
            {
                return 0;
            }
        } else if (self.pawn(me) & self.line(me, 5) & (t.file[0] | t.file[7])) != 0 {
            let sq = lsb(self.pawn(me));
            let p = (sq as i32 + Self::push(me)) as usize;
            if self.pos.board.square[p] == ipawn(opp(me)) as u8 {
                if (t.s_area[p] & self.king(opp(me)) & self.line(me, 7)) != 0 {
                    return 0;
                }
                if (t.s_area[p] & t.s_area[lsb(self.king(opp(me)))] & self.line(me, 7)) != 0
                    && ((t.n_att[p] & self.knight(me)) == 0
                        || self.pos.data[self.pos.cur].turn as Color == opp(me))
                {
                    return 0;
                }
            }
        }
        32
    }

    fn krpkrx(&self, me: Color) -> i32 {
        let t = self.tables;
        let cur = self.pos.cur;
        let oppc = opp(me);
        let mut mul = 32;
        let sq = lsb(self.pawn(me));
        let rrank = crank(me, sq);
        let o_king = lsb(self.king(oppc));
        let o_rook = lsb(self.rook(oppc));
        let m_king = lsb(self.king(me));
        let add_mat = (self.piece(oppc) ^ self.king(oppc) ^ self.rook(oppc)) != 0;
        let clear = !add_mat
            || ((t.p_way[oppc][sq] | t.p_isolated[file(sq)])
                & t.forward[oppc][rank((sq as i32 + Self::push(me)) as usize)]
                & (self.piece(oppc) ^ self.king(oppc) ^ self.rook(oppc)))
                == 0;
        if !clear {
            return 32;
        }
        if !add_mat && (self.pawn(me) & (t.file[0] | t.file[7])) == 0 {
            let m_rook = lsb(self.rook(me));
            if crank(me, o_king) < crank(me, m_rook)
                && crank(me, m_rook) < rrank
                && crank(me, m_king) >= rrank.wrapping_sub(1)
                && crank(me, m_king) > crank(me, m_rook)
                && ((t.s_area[m_king] & self.pawn(me)) != 0
                    || (self.pos.data[cur].turn as Color == me
                        && (file(sq) as i32 - file(m_king) as i32).abs() <= 1
                        && (rrank as i32 - crank(me, m_king) as i32).abs() <= 2))
            {
                return 128;
            }
            if (t.s_area[m_king] & self.pawn(me)) != 0 {
                if rrank >= 4 {
                    if (file(sq) < file(m_rook) && file(m_rook) < file(o_king))
                        || (file(sq) > file(m_rook) && file(m_rook) > file(o_king))
                    {
                        return 128;
                    }
                } else if rrank >= 2 {
                    if (self.pawn(me) & (t.file[1] | t.file[6])) == 0
                        && rrank + (file(sq) as i32 - file(m_rook) as i32).abs() as usize > 4
                        && ((file(sq) < file(m_rook) && file(m_rook) < file(o_king))
                            || (file(sq) > file(m_rook) && file(m_rook) > file(o_king)))
                    {
                        return 128;
                    }
                }
            }
        }
        if (t.p_way[me][sq] & self.king(oppc)) != 0 {
            if (self.pawn(me) & (t.file[0] | t.file[7])) != 0 {
                mul = mul.min(if add_mat { 8 } else { 0 });
            }
            if rrank <= 3 {
                mul = mul.min(if add_mat { 8 } else { 0 });
            }
            if rrank == 4
                && crank(me, m_king) <= 4
                && crank(me, o_rook) == 5
                && (self.king(oppc) & (self.line(me, 6) | self.line(me, 7))) != 0
                && (self.pos.data[cur].turn as Color != me
                    || (t.p_att[me][sq]
                        & t.rook_attacks(lsb(self.rook(me)), self.piece_all())
                        & (!t.s_area[o_king]))
                        == 0)
            {
                mul = mul.min(if add_mat { 8 } else { 0 });
            }
            if rrank >= 5
                && crank(me, o_rook) <= 1
                && (self.pos.data[cur].turn as Color != me
                    || self.check(me)
                    || dist(m_king, sq) >= 2)
            {
                mul = mul.min(if add_mat { 8 } else { 0 });
            }
            if (self.king(oppc) & (t.file[1] | t.file[2] | t.file[6] | t.file[7])) != 0
                && (self.rook(oppc) & self.line(me, 7)) != 0
                && (t.between[o_king][o_rook] & (t.file[3] | t.file[4])) != 0
                && (self.rook(me) & self.line(me, 7)) == 0
            {
                mul = mul.min(if add_mat { 8 } else { 0 });
            }
            mul
        } else if rrank == 6
            && (self.pawn(me) & (t.file[0] | t.file[7])) != 0
            && ((t.p_support[me][sq] | t.p_way[oppc][sq]) & self.rook(oppc)) != 0
            && crank(me, o_king) >= 6
        {
            let d = (file(sq) as i32 - file(o_king) as i32).abs();
            if d <= 3 {
                mul = mul.min(if add_mat { 8 } else { 0 });
            }
            if d == 4
                && ((t.p_support[me][o_king] & self.rook(me)) != 0
                    || self.pos.data[cur].turn as Color == oppc)
            {
                mul = mul.min(if add_mat { 8 } else { 0 });
            }
            mul
        } else {
            if (t.s_area[o_king] & t.p_way[me][sq] & self.line(me, 7)) != 0 {
                if rrank <= 4 && crank(me, m_king) <= 4 && crank(me, o_rook) == 5 {
                    mul = mul.min(if add_mat { 8 } else { 0 });
                }
                if (rrank == 5 && crank(me, o_rook) <= 1 && self.pos.data[cur].turn as Color != me)
                    || ((t.s_area[m_king] & t.p_att[me][sq] & (!t.s_area[o_king])) == 0
                        && (self.check(me) || dist(m_king, sq) >= 2))
                {
                    mul = mul.min(if add_mat { 8 } else { 0 });
                }
            }
            if (t.p_way[me][sq] & self.rook(me)) != 0 && (t.p_way[oppc][sq] & self.rook(oppc)) != 0
            {
                if (self.king(oppc)
                    & (t.file[0] | t.file[1] | t.file[6] | t.file[7])
                    & self.line(me, 6))
                    != 0
                {
                    mul = mul.min(if add_mat { 8 } else { 0 });
                } else if (self.pawn(me) & (t.file[0] | t.file[7])) != 0
                    && (self.king(oppc) & (self.line(me, 5) | self.line(me, 6))) != 0
                    && (file(sq) as i32 - file(o_king) as i32).abs() <= 2
                    && file(sq) != file(o_king)
                {
                    mul = mul.min(if add_mat { 8 } else { 0 });
                }
            }
            if (file(sq) as i32 - file(o_king) as i32).abs() <= 1
                && (file(sq) as i32 - file(o_rook) as i32).abs() <= 1
                && crank(me, o_rook) > rrank
                && crank(me, o_king) > rrank
            {
                mul = mul.min(if (self.pawn(me) & (t.file[3] | t.file[4])) != 0 {
                    12
                } else {
                    16
                });
            }
            mul
        }
    }

    fn krpkbx(&self, me: Color) -> i32 {
        let t = self.tables;
        let cur = self.pos.cur;
        let oppc = opp(me);
        if (self.pawn(me) & self.line(me, 5)) == 0 {
            return 32;
        }
        let sq = lsb(self.pawn(me));
        if (t.p_way[me][sq] & self.king(oppc)) == 0 {
            return 32;
        }
        let p = (sq as i32 + Self::push(me)) as usize;
        let diag_sq = nb(me, t.b_mask[p]);
        if crank(me, diag_sq) > 1 {
            return 32;
        }
        let mdiag = t.full_line[p][diag_sq] | bit(p) | bit(diag_sq);
        let q = (sq as i32 - Self::push(me)) as usize;
        let check_sq = nb(me, t.b_mask[q]);
        let cdiag = t.full_line[q][check_sq] | bit(q) | bit(check_sq);
        if ((mdiag | cdiag) & (self.piece(oppc) ^ self.king(oppc) ^ self.bishop(oppc))) != 0 {
            return 32;
        }
        if (cdiag & self.bishop(oppc)) != 0 {
            return 0;
        }
        if (mdiag & self.bishop(oppc)) != 0
            && (self.pos.data[cur].turn as Color == oppc || (self.king(me) & t.p_att[oppc][p]) == 0)
        {
            return 0;
        }
        32
    }

    fn kqkp(&self, me: Color) -> i32 {
        let t = self.tables;
        let oppc = opp(me);
        if (t.s_area[lsb(self.king(oppc))]
            & self.pawn(oppc)
            & self.line(me, 1)
            & (t.file[0] | t.file[2] | t.file[5] | t.file[7]))
            == 0
        {
            return 32;
        }
        if (t.p_way[oppc][lsb(self.pawn(oppc))] & (self.king(me) | self.queen(me))) != 0 {
            return 32;
        }
        if (self.pawn(oppc) & (t.file[0] | t.file[7])) != 0 {
            1
        } else {
            4
        }
    }

    fn kqkrpx(&self, me: Color) -> i32 {
        let t = self.tables;
        let oppc = opp(me);
        let rsq = lsb(self.rook(oppc));
        let pawns = t.s_area[lsb(self.king(oppc))]
            & t.p_att[me][rsq]
            & self.pawn(oppc)
            & INTERIOR
            & self.line(me, 6);
        if pawns != 0 && crank(me, lsb(self.king(me))) <= 4 {
            return 0;
        }
        32
    }

    fn krkpx(&self, me: Color) -> i32 {
        let t = self.tables;
        let oppc = opp(me);
        if (t.s_area[lsb(self.king(oppc))] & self.pawn(oppc) & self.line(me, 1)) != 0
            && (t.p_way[oppc][nb(me, self.pawn(oppc))] & self.king(me)) == 0
        {
            return 0;
        }
        32
    }

    fn krppkrpx(&self, me: Color) -> i32 {
        let t = self.tables;
        let cur = self.pos.cur;
        let oppc = opp(me);
        if (self.pos.data[cur].passer & self.pawn(me)) != 0 {
            if single(self.pos.data[cur].passer & self.pawn(me)) {
                let sq = lsb(self.pos.data[cur].passer & self.pawn(me));
                if (t.p_way[me][sq]
                    & self.king(oppc)
                    & (t.file[0] | t.file[1] | t.file[6] | t.file[7]))
                    != 0
                {
                    let opp_king = lsb(self.king(oppc));
                    if (t.s_area[opp_king] & self.pawn(oppc)) != 0 {
                        let kf = file(opp_king);
                        if ((!(t.file[kf] | t.p_isolated[kf])) & self.pawn(me)) != 0 {
                            return 1;
                        }
                    }
                }
            }
            return 32;
        }
        if ((!(t.p_way[oppc][lsb(self.king(oppc))] | t.p_support[me][lsb(self.king(oppc))]))
            & self.pawn(me))
            == 0
        {
            return 0;
        }
        32
    }

    fn krpppkrppx(&self, me: Color) -> i32 {
        let t = self.tables;
        let oppc = opp(me);
        if (self.pos.data[self.pos.cur].passer & self.pawn(me)) != 0
            || (t.s_area[lsb(self.pawn(oppc))] | t.s_area[msb(self.pawn(oppc))]) & self.pawn(oppc)
                == 0
        {
            return 32;
        }
        if ((!(t.p_way[oppc][lsb(self.king(oppc))] | t.p_support[me][lsb(self.king(oppc))]))
            & self.pawn(me))
            == 0
        {
            return 0;
        }
        32
    }

    fn kbpkbx(&self, me: Color) -> i32 {
        let t = self.tables;
        let cur = self.pos.cur;
        let oppc = opp(me);
        let sq = lsb(self.pawn(me));
        if ((self.pos.board.bb[ilight(me)] != 0 && self.pos.board.bb[idark(oppc)] != 0)
            || (self.pos.board.bb[idark(me)] != 0 && self.pos.board.bb[ilight(oppc)] != 0))
        {
            if crank(me, sq) <= 4 {
                return 0;
            }
            if (t.p_way[me][sq] & self.king(oppc)) != 0 && crank(me, sq) <= 5 {
                return 0;
            }
            let mut u = self.bishop(oppc);
            while u != 0 {
                let s = lsb(u);
                cut(&mut u);
                if crank(me, s) <= 4
                    && (t.bishop_attacks(s, self.piece_all()) & t.p_way[me][sq]) != 0
                {
                    return 0;
                }
                if self.pos.data[cur].turn as Color == oppc
                    && (t.bishop_attacks(s, self.piece_all()) & self.pawn(me)) != 0
                {
                    return 0;
                }
            }
        } else if (t.p_way[me][sq] & self.king(oppc)) != 0
            && ((self.king(oppc) & LIGHT_AREA) != 0) != ((self.bishop(me) & LIGHT_AREA) != 0)
        {
            return 0;
        }
        32
    }

    fn kbpknx(&self, me: Color) -> i32 {
        let t = self.tables;
        let cur = self.pos.cur;
        let oppc = opp(me);
        if (t.p_way[me][lsb(self.pawn(me))] & self.king(oppc)) != 0
            && ((self.king(oppc) & LIGHT_AREA) != 0) != ((self.bishop(me) & LIGHT_AREA) != 0)
        {
            return 0;
        }
        if self.pos.data[cur].turn as Color == oppc {
            let mut u = self.knight(oppc);
            while u != 0 {
                let s = lsb(u);
                cut(&mut u);
                if (t.n_att[s] & self.pawn(me)) != 0 {
                    return 0;
                }
            }
        }
        32
    }

    fn kbppkbx(&self, me: Color) -> i32 {
        let t = self.tables;
        let oppc = opp(me);
        let sq1 = nb(me, self.pawn(me));
        let sq2 = nb(oppc, self.pawn(me));
        let o_king = lsb(self.king(oppc));
        let o_bishop = lsb(self.bishop(oppc));
        if file(sq1) == file(sq2) {
            if crank(me, sq2) <= 3 {
                return 0;
            }
            if (t.p_way[me][sq2] & self.king(oppc)) != 0 && crank(me, sq2) <= 5 {
                return 0;
            }
        } else if (t.p_isolated[file(sq1)] & self.pawn(me)) != 0 {
            if ((self.king(oppc) & LIGHT_AREA) != 0) != ((self.bishop(me) & LIGHT_AREA) != 0) {
                let target = (sq2 as i32 + Self::push(me)) as usize;
                if ((t.s_area[o_king] | self.king(oppc)) & bit(target)) != 0
                    && (t.bishop_attacks(o_bishop, self.piece_all()) & bit(target)) != 0
                {
                    let target2 = (sq2 & !7usize) | file(sq1);
                    if ((t.s_area[o_king] | self.king(oppc)) & bit(target2)) != 0
                        && (t.bishop_attacks(o_bishop, self.piece_all()) & bit(target2)) != 0
                    {
                        return 0;
                    }
                }
            }
        }
        32
    }

    fn krppkrx(&self, me: Color) -> i32 {
        let t = self.tables;
        let oppc = opp(me);
        let sq1 = nb(me, self.pawn(me));
        let sq2 = nb(oppc, self.pawn(me));
        if ((self.piece(oppc) ^ self.king(oppc) ^ self.rook(oppc))
            & t.forward[me][rank((sq1 as i32 - Self::push(me)) as usize)])
            != 0
        {
            return 32;
        }
        if file(sq1) == file(sq2) {
            if (t.p_way[me][sq2] & self.king(oppc)) != 0 {
                return 16;
            }
            return 32;
        }
        if (t.p_isolated[file(sq2)] & self.pawn(me)) != 0
            && ((t.file[0] | t.file[7]) & self.pawn(me)) != 0
            && (self.king(oppc) & Self::shift(me, self.pawn(me))) != 0
        {
            if crank(me, sq2) == 5
                && crank(me, sq1) == 4
                && (self.rook(oppc) & (self.line(me, 5) | self.line(me, 6))) != 0
            {
                return 10;
            } else if crank(me, sq2) < 5 {
                return 16;
            }
        }
        32
    }

    fn krbkrx(&self, me: Color) -> i32 {
        let _ = me;
        if (self.king(opp(me)) & INTERIOR) != 0 {
            1
        } else {
            16
        }
    }
}

// ---------------------------------------------------------------------------
// Pawn structure evaluation
// ---------------------------------------------------------------------------

fn eval_pawn_structure(pos: &Position, t: &Tables, pe: &mut PawnEntry) {
    *pe = PawnEntry::default();
    pe.key = (pos.data[pos.cur].pawn_key >> 16) & 0x0000_FFFF_FFFF_FFFF;
    let mut pei = PawnEvalInfo {
        king_w: lsb(pos.board.bb[WHITE_KING]),
        king_b: lsb(pos.board.bb[BLACK_KING]),
        score: 0,
        patt_w: Engine::shift_w(WHITE, pos.board.bb[WHITE_PAWN])
            | Engine::shift_e(WHITE, pos.board.bb[WHITE_PAWN]),
        patt_b: Engine::shift_w(BLACK, pos.board.bb[BLACK_PAWN])
            | Engine::shift_e(BLACK, pos.board.bb[BLACK_PAWN]),
        double_att_w: Engine::shift_w(WHITE, pos.board.bb[WHITE_PAWN])
            & Engine::shift_e(WHITE, pos.board.bb[WHITE_PAWN]),
        double_att_b: Engine::shift_w(BLACK, pos.board.bb[BLACK_PAWN])
            & Engine::shift_e(BLACK, pos.board.bb[BLACK_PAWN]),
    };
    eval_pawns(WHITE, pos, t, pe, &mut pei);
    eval_pawns(BLACK, pos, t, pe, &mut pei);
    pe.score = pei.score;
}

#[inline(always)]
fn pawn_att(ei: &PawnEvalInfo, me: Color) -> u64 {
    if me == WHITE {
        ei.patt_w
    } else {
        ei.patt_b
    }
}

#[inline(always)]
fn double_att(ei: &PawnEvalInfo, me: Color) -> u64 {
    if me == WHITE {
        ei.double_att_w
    } else {
        ei.double_att_b
    }
}

#[inline(always)]
fn pawn_king(ei: &PawnEvalInfo, me: Color) -> usize {
    if me == WHITE {
        ei.king_w
    } else {
        ei.king_b
    }
}

fn eval_pawns(me: Color, pos: &Position, t: &Tables, pe: &mut PawnEntry, pei: &mut PawnEvalInfo) {
    let oppc = opp(me);
    let board = &pos.board;
    let pawn_me = board.bb[WHITE_PAWN | me];
    let pawn_opp = board.bb[WHITE_PAWN | oppc];
    let pawn_all = board.bb[WHITE_PAWN] | board.bb[BLACK_PAWN];
    let kf = file(pawn_king(pei, me));
    let kr = rank(pawn_king(pei, me));
    let (start, inc) = if kf <= 3 {
        (kf.saturating_sub(1), 1i32)
    } else {
        ((kf + 1).min(7), -1i32)
    };
    let mut shelter = 0i32;
    let mpawns = pawn_me
        & t.forward[me][if me == BLACK {
            (kr + 1).min(7)
        } else {
            kr.saturating_sub(1)
        }];
    let mut f = start as i32;
    for i in 0..3 {
        let file_idx = f as usize;
        shelter += t.shelter[i][crank(me, nbz(me, mpawns & t.file[file_idx]))];
        if (pawn_opp & t.file[file_idx]) != 0 {
            let sq = nb(me, pawn_opp & t.file[file_idx]);
            let r = crank(oppc, sq);
            if r < 6 {
                if r >= 3 {
                    shelter += t.storm_blocked[r - 3];
                }
                let u = t.p_isolated[file(sq)] & t.forward[oppc][rank(sq)] & pawn_me;
                if u != 0 {
                    let square = nb(oppc, u);
                    let att_sq = t.p_att[me][square] & t.p_way[oppc][sq];
                    if ((t.file[file(square)] | t.p_isolated[file(square)])
                        & board.bb[WHITE_KING | me])
                        != 0
                        && ((double_att(pei, me) & att_sq) == 0
                            || (pos.data[pos.cur].patt[oppc] & att_sq) != 0)
                    {
                        if (t.p_way[oppc][square] & pawn_me) != 0 {
                            f += inc;
                            continue;
                        }
                        if (pawn_all & t.p_way[oppc][sq] & t.forward[me][rank(square)]) == 0 {
                            if r >= 3 {
                                shelter += t.storm_shelter_att[r - 3];
                                if (pawn_att(pei, oppc)
                                    & bit((sq as i32 + Engine::push(oppc)) as usize))
                                    != 0
                                {
                                    shelter += t.storm_connected[r - 3];
                                }
                                if (t.p_way[oppc][sq] & pawn_all) == 0 {
                                    shelter += t.storm_open[r - 3];
                                }
                            }
                            if ((t.file[file(sq)] | t.p_isolated[file(sq)])
                                & board.bb[WHITE_KING | oppc])
                                == 0
                                && r <= 4
                            {
                                shelter += t.storm_free[r - 1];
                            }
                        }
                    }
                }
            }
        } else {
            shelter += t.storm_hof[STORM_HOF_VALUE];
            if (pawn_me & t.file[file_idx]) == 0 {
                shelter += t.storm_hof[STORM_OF_VALUE];
            }
        }
        f += inc;
    }
    pe.shelter[me] = if shelter > 255 { 255 } else { shelter as u8 };
    pe.passer[me] = 0;

    let mut u = pawn_me;
    while u != 0 {
        let sq = lsb(u);
        let b = bit(sq);
        u ^= b;
        let r = rank(sq);
        let rrank = crank(me, sq);
        let f = file(sq);
        let way = t.p_way[me][sq];
        let next = board.square[(sq as i32 + Engine::push(me)) as usize] as usize;
        let isolated = (pawn_me & t.p_isolated[f]) == 0;
        let doubled = (pawn_me & (t.file[f] ^ b)) != 0;
        let open = (pawn_all & way) == 0;
        let up = (pawn_att(pei, me) & b) == 0;

        if isolated {
            if open {
                decv(me, &mut pei.score, ca(&t.isolated, ISOLATED_OPEN));
            } else {
                decv(me, &mut pei.score, ca(&t.isolated, ISOLATED_CLOSED));
                if next == ipawn(oppc) {
                    decv(me, &mut pei.score, ca(&t.isolated, ISOLATED_BLOCKED));
                }
            }
            if doubled {
                if open {
                    decv(me, &mut pei.score, ca(&t.isolated, ISOLATED_DOUBLED_OPEN));
                } else {
                    decv(me, &mut pei.score, ca(&t.isolated, ISOLATED_DOUBLED_CLOSED));
                }
            }
        } else {
            if doubled {
                if open {
                    decv(me, &mut pei.score, ca(&t.doubled, DOUBLED_OPEN));
                } else {
                    decv(me, &mut pei.score, ca(&t.doubled, DOUBLED_CLOSED));
                }
            }
            if rrank >= 3
                && (b & (t.file[2] | t.file[3] | t.file[4] | t.file[5])) != 0
                && next != ipawn(oppc)
                && (t.p_isolated[f] & t.line[r] & pawn_me) != 0
            {
                incv(
                    me,
                    &mut pei.score,
                    ca(&t.pawn_special, PAWN_CHAIN_LINEAR) * (rrank as i32 - 3)
                        + ca(&t.pawn_special, PAWN_CHAIN),
                );
            }
        }
        let mut backward = false;
        if (t.p_support[me][sq] & pawn_me) == 0 {
            if isolated {
                backward = true;
            } else {
                let v = (pawn_all | pawn_att(pei, oppc)) & way;
                if v != 0 && Engine::is_greater(me, nb(me, pawn_att(pei, me) & way), nb(me, v)) {
                    backward = true;
                }
            }
        }
        if backward {
            if open {
                decv(me, &mut pei.score, ca(&t.backward, BACKWARD_OPEN));
            } else {
                decv(me, &mut pei.score, ca(&t.backward, BACKWARD_CLOSED));
            }
        } else if open
            && ((pawn_opp & t.p_isolated[f]) == 0
                || popcount(pawn_me & t.p_isolated[f]) >= popcount(pawn_opp & t.p_isolated[f]))
        {
            incv(me, &mut pei.score, t.passer_candidate[rrank]);
        }
        if up && next == ipawn(oppc) {
            decv(me, &mut pei.score, ca(&t.unprotected, UP_BLOCKED));
            if backward {
                if rrank <= 2 {
                    decv(me, &mut pei.score, ca(&t.unprotected, PASSER_TARGET));
                    if rrank <= 1 {
                        decv(me, &mut pei.score, ca(&t.unprotected, PASSER_TARGET));
                    }
                }
                let mut v = t.p_att[me][sq] & pawn_me;
                while v != 0 {
                    let s = lsb(v);
                    cut(&mut v);
                    if (t.p_support[me][s] & pawn_me) == b {
                        decv(me, &mut pei.score, ca(&t.unprotected, CHAIN_ROOT));
                        break;
                    }
                }
            }
        }
        if open && (t.p_isolated[f] & t.forward[me][r] & pawn_opp) == 0 {
            pe.passer[me] |= 1u8 << f;
            if rrank <= 2 {
                continue;
            }
            incv(me, &mut pei.score, t.passer_general[rrank]);
            let dist_att = dist(
                pawn_king(pei, oppc),
                (sq as i32 + Engine::push(me)) as usize,
            );
            let dist_def = dist(pawn_king(pei, me), (sq as i32 + Engine::push(me)) as usize);
            incv(
                me,
                &mut pei.score,
                compose256(
                    0,
                    dist_att * t.passer_att[rrank]
                        + t.log_dist[dist_att as usize] * t.passer_att_log[rrank]
                        - dist_def * t.passer_def[rrank]
                        - t.log_dist[dist_def as usize] * t.passer_def_log[rrank],
                ),
            );
            if (pawn_att(pei, me) & b) != 0 {
                incv(me, &mut pei.score, t.passer_protected[rrank]);
            }
            if (pawn_opp & t.west[f]) == 0 || (pawn_opp & t.east[f]) == 0 {
                incv(me, &mut pei.score, t.passer_outside[rrank]);
            }
        }
    }
    let mut files = 0u64;
    for i in 1..7 {
        files |= (pawn_me >> (i << 3)) & 0xFF;
    }
    let span = if files != 0 {
        msb(files) as i32 - lsb(files) as i32
    } else {
        0
    };
    incv(
        me,
        &mut pei.score,
        ca(&t.pawn_special, PAWN_FILE_SPAN) * span,
    );
    pe.draw[me] = (((7 - span) * (5 - popcount(files)).max(0)) as u8) as u8;
}

// Keep rustc from warning about the unused `pvar_free` shim.
#[allow(dead_code)]
fn _unused(ei: &EvalInfo) -> u64 {
    pvar_free(ei, WHITE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Engine;

    #[test]
    fn start_position_is_balanced() {
        let mut e = Engine::with_hash_mb(1);
        e.evaluate();
        assert_eq!(e.pos.cur().score, 0);
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
            e.get_board(fen);
            e.evaluate();
            let white_score = e.pos.cur().score;
            // Flip the board colours and side to move.
            let flipped = flip_fen(fen);
            let mut e2 = Engine::with_hash_mb(1);
            e2.get_board(&flipped);
            e2.evaluate();
            assert_eq!(white_score, e2.pos.cur().score, "{fen} vs {flipped}");
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
        e.get_board(fen);
        e.evaluate();
        e.pos.cur().score as i32
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
