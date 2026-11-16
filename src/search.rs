//! Search: alpha/beta, quiescence, move ordering, hash tables and the
//! iterative-deepening root.
//!
//! This module is an exact port of the search in the C++ Gull source.  The
//! only structural differences are Rust idioms (no `goto`, no `setjmp`); the
//! control flow, move ordering scores, hash replacement rules and node
//! counting follow the original operation by operation.

use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::position::{Engine, State};
use crate::types::*;

pub const TIME_SING_TWO_MARGIN: u64 = 20;
pub const TIME_SING_ONE_MARGIN: u64 = 30;
pub const TIME_NO_PVSCO_MARGIN: u64 = 60;
pub const TIME_NO_CHANGE_MARGIN: u64 = 70;
pub const TIME_RATIO: u64 = 120;
pub const PONDER_RATIO: u64 = 120;
pub const MOVES_TG: i64 = 30;
pub const INFO_LAG: u64 = 5000;
pub const INFO_DELAY: u64 = 1000;

pub const EXCL_SINGLE: i32 = 8;
pub const EXCL_DOUBLE: i32 = 16;
pub const EXCL_SINGLE_PV: i32 = 8;
pub const EXCL_DOUBLE_PV: i32 = 16;

pub const REF_ONE_SCORE: i32 = (0xFF << 16) | (3 << 24);
pub const REF_TWO_SCORE: i32 = (0xFF << 16) | (2 << 24);
pub const KILLER_ONE_SCORE: i32 = (0xFF << 16) | (1 << 24);
pub const KILLER_TWO_SCORE: i32 = 0xFF << 16;

pub const MAX_PV_LEN: usize = 1024;
pub const INFINITE_TIME: u64 = u32::MAX as u64;

/// Payload used to unwind a search when it is stopped.
pub struct SearchAborted;

#[inline]
pub fn now_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis() as u64,
        Err(_) => 0,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchInfo {
    pub bad: i32,
    pub change: i32,
    pub singular: i32,
    pub early: i32,
    pub fail_low: i32,
    pub fail_high: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchStats {
    pub depth: i32,
    pub sel_depth: i32,
    pub best_move: i32,
    pub best_score: i32,
    pub id: i32,
    pub stop: bool,
    pub new_game: bool,
    pub pv: [i32; 64],
}

impl Default for SearchStats {
    fn default() -> Self {
        SearchStats {
            depth: 0,
            sel_depth: 0,
            best_move: 0,
            best_score: 0,
            id: 0,
            stop: false,
            new_game: false,
            pv: [0; 64],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SharedInfo {
    pub date: u16,
    pub root_depth: u32,
    pub depth_limit: u32,
    pub node_limit: u64,
    pub start_time: u64,
    pub soft_time_limit: u64,
    pub hard_time_limit: u64,
    pub ponder: bool,
    pub root_board: crate::position::Board,
    pub root_state: State,
    pub root_sp: usize,
    pub root_stack: [u64; STACK_SIZE],
}

impl Default for SharedInfo {
    fn default() -> Self {
        SharedInfo {
            date: 0,
            root_depth: 1,
            depth_limit: 512,
            node_limit: 0,
            start_time: 0,
            soft_time_limit: u64::MAX,
            hard_time_limit: u64::MAX,
            ponder: false,
            root_board: crate::position::Board::default(),
            root_state: State::default(),
            root_sp: 0,
            root_stack: [0; STACK_SIZE],
        }
    }
}

/// External stop signal for a search running on another thread.
#[derive(Clone, Debug)]
pub struct SearchControl {
    pub stop: Arc<AtomicBool>,
}

impl Default for SearchControl {
    fn default() -> Self {
        SearchControl {
            stop: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl SearchControl {
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
    pub fn is_stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
    pub fn reset(&self) {
        self.stop.store(false, Ordering::Relaxed);
    }
}

/// Information handed to a [`Reporter`] when a PV is to be printed.
#[derive(Clone, Debug, Default)]
pub struct Info {
    /// MultiPV index (1-based); `0` for a normal single-PV search.
    pub multipv: usize,
    pub depth: i32,
    pub sel_depth: i32,
    pub score: i32,
    pub nodes: u64,
    pub tb_hits: u64,
    pub time_ms: u64,
    pub pv: Vec<i32>,
}

/// Output sink used by the search.  The UCI layer implements this trait.
pub trait Reporter: Send {
    fn info_depth(&mut self, _depth: i32) {}
    fn curr_move(&mut self, _mv: i32, _cnt: i32, _time_ms: u64, _nodes: u64) {}
    fn pv(&mut self, _info: &Info) {}
}

/// Discards all search output.
pub struct NullReporter;

impl Reporter for NullReporter {}

#[derive(Clone, Debug, Default)]
pub struct SearchResult {
    pub best_move: i32,
    pub ponder_move: i32,
    pub score: i32,
    pub nodes: u64,
    pub depth: i32,
    pub sel_depth: i32,
    pub pv: Vec<i32>,
}

#[derive(Clone, Debug)]
pub struct SearchLimits {
    /// The UCI `depth` limit; internal depth limit is `2 * depth + 2`.
    pub depth: u32,
    /// Hard time limit in milliseconds.
    pub movetime: Option<u64>,
    pub nodes: u64,
    pub infinite: bool,
    pub ponder: bool,
    /// Time management inputs (milliseconds).
    pub wtime: u64,
    pub btime: u64,
    pub winc: u64,
    pub binc: u64,
    pub movestogo: u64,
}

impl Default for SearchLimits {
    fn default() -> Self {
        SearchLimits {
            depth: 256,
            movetime: None,
            nodes: 0,
            infinite: false,
            ponder: false,
            wtime: 0,
            btime: 0,
            winc: 0,
            binc: 0,
            movestogo: 0,
        }
    }
}

impl SearchLimits {
    pub fn depth(d: u32) -> SearchLimits {
        SearchLimits {
            depth: d,
            ..SearchLimits::default()
        }
    }
}

#[inline(always)]
fn hash_key_low(key: u64) -> u32 {
    key as u32
}

#[inline(always)]
fn hash_key_high(key: u64) -> u32 {
    (key >> 32) as u32
}

impl Engine {
    // -- evaluation / check helpers ----------------------------------------

    #[inline(always)]
    pub fn check(&self, me: Color) -> bool {
        (self.pos.data[self.pos.cur].att[opp(me)] & self.king(me)) != 0
    }

    /// `IsIllegal(me, move)` from the C++ source.
    #[inline(always)]
    pub fn is_illegal(&self, me: Color, mv: Move) -> bool {
        let from = move_from(mv);
        let to = move_to(mv);
        let st = &self.pos.data[self.pos.cur];
        if (st.xray[opp(me)] & bit(from)) != 0
            && (bit(to) & self.tables.full_line[lsb(self.king(me))][from]) == 0
        {
            return true;
        }
        if is_ep(mv) {
            let ks = lsb(self.king(me));
            if (self.tables.line[rank(from)] & self.king(me)) != 0
                && (self.tables.line[rank(from)] & self.major(opp(me))) != 0
            {
                let cap_sq = (st.ep_square as i32 - Self::push(me)) as usize;
                let occ = self.piece_all() ^ bit(from) ^ bit(cap_sq);
                if (self.tables.rook_attacks(ks, occ) & self.major(opp(me))) != 0 {
                    return true;
                }
            }
        }
        false
    }

    /// `is_legal<me>` from the C++ source.
    #[inline(always)]
    pub fn is_legal(&mut self, me: Color, mv: Move) -> bool {
        let from = move_from(mv);
        let to = move_to(mv);
        let piece = self.pos.board.square[from] as usize;
        let capture = self.pos.board.square[to] as usize;
        let st = &self.pos.data[self.pos.cur];
        if piece == 0 {
            return false;
        }
        if (piece & 1) != st.turn as usize {
            return false;
        }
        if capture != 0 {
            if (capture & 1) == (piece & 1) {
                return false;
            }
            if capture >= WHITE_KING {
                return false;
            }
        }
        let occ = self.piece_all();
        let u = bit(to);
        if piece >= WHITE_LIGHT && piece < WHITE_KING {
            if (self.tables.q_mask[from] & u) == 0 {
                return false;
            }
            if (self.tables.between[from][to] & occ) != 0 {
                return false;
            }
        }
        if is_ep(mv) {
            if piece >= WHITE_KNIGHT {
                return false;
            }
            if st.ep_square as usize != to {
                return false;
            }
            return true;
        }
        if is_castling(mv) && self.pos.board.square[from] < WHITE_KING as u8 {
            return false;
        }
        if is_promotion(mv) && self.pos.board.square[from] >= WHITE_KNIGHT as u8 {
            return false;
        }
        if piece == ipawn(me) {
            if (u & self.tables.p_move[me][from]) != 0 {
                if capture != 0 {
                    return false;
                }
                if (u & self.line(me, 7)) != 0 && !is_promotion(mv) {
                    return false;
                }
                return true;
            } else if to as i32 == from as i32 + 2 * Self::push(me) {
                if capture != 0 {
                    return false;
                }
                if self.pos.board.square[(to as i32 - Self::push(me)) as usize] != 0 {
                    return false;
                }
                if (u & self.line(me, 3)) == 0 {
                    return false;
                }
                return true;
            } else if (u & self.tables.p_att[me][from]) != 0 {
                if capture == 0 {
                    return false;
                }
                if (u & self.line(me, 7)) != 0 && !is_promotion(mv) {
                    return false;
                }
                return true;
            } else {
                return false;
            }
        } else if piece == iking(me) {
            if me == WHITE {
                if is_castling(mv) {
                    if (u & 0x40) != 0 {
                        if (st.castle_flags & CAN_CASTLE_OO) == 0 {
                            return false;
                        }
                        if (occ & 0x60) != 0 {
                            return false;
                        }
                        if (st.att[BLACK] & 0x70) != 0 {
                            return false;
                        }
                    } else {
                        if (st.castle_flags & CAN_CASTLE_OOO) == 0 {
                            return false;
                        }
                        if (occ & 0xE) != 0 {
                            return false;
                        }
                        if (st.att[BLACK] & 0x1C) != 0 {
                            return false;
                        }
                    }
                    return true;
                }
            } else if is_castling(mv) {
                if (u & 0x4000_0000_0000_0000) != 0 {
                    if (st.castle_flags & CAN_CASTLE_oo) == 0 {
                        return false;
                    }
                    if (occ & 0x6000_0000_0000_0000) != 0 {
                        return false;
                    }
                    if (st.att[WHITE] & 0x7000_0000_0000_0000) != 0 {
                        return false;
                    }
                } else {
                    if (st.castle_flags & CAN_CASTLE_ooo) == 0 {
                        return false;
                    }
                    if (occ & 0x0E00_0000_0000_0000) != 0 {
                        return false;
                    }
                    if (st.att[WHITE] & 0x1C00_0000_0000_0000) != 0 {
                        return false;
                    }
                }
                return true;
            }
            if (self.tables.s_area[from] & u) == 0 {
                return false;
            }
            if (st.att[opp(me)] & u) != 0 {
                return false;
            }
            return true;
        }
        let pt = (piece >> 1) - 2;
        let legal = if pt == 0 {
            (u & self.tables.n_att[from]) != 0
        } else if pt <= 2 {
            (self.tables.b_mask[from] & u) != 0
        } else if pt == 3 {
            (self.tables.r_mask[from] & u) != 0
        } else {
            true
        };
        if !legal {
            return false;
        }
        // Full legality check: king must not be in check after the move.
        self.do_move(me, mv);
        self.evaluate();
        let in_check = if me == WHITE {
            (self.pos.data[self.pos.cur].att[BLACK] & self.king(WHITE)) != 0
        } else {
            (self.pos.data[self.pos.cur].att[WHITE] & self.king(BLACK)) != 0
        };
        self.undo_move(me, mv);
        !in_check
    }

    /// `is_check<me>`: does not detect castling or en-passant checks.
    pub fn is_check(&self, me: Color, mv: Move) -> bool {
        let from = move_from(mv);
        let to = move_to(mv);
        let king = self.king(opp(me));
        let king_sq = lsb(king);
        let piece = self.pos.board.square[from] as usize;
        let st = &self.pos.data[self.pos.cur];
        if (bit(from) & st.xray[me]) != 0 && (self.tables.full_line[king_sq][from] & bit(to)) == 0 {
            return true;
        }
        if piece < WHITE_KNIGHT {
            if (self.tables.p_att[me][to] & king) != 0 {
                return true;
            }
            if (bit(to) & self.line(me, 7)) != 0
                && (king & self.line(me, 7)) != 0
                && (self.tables.between[to][king_sq] & self.piece_all()) == 0
            {
                return true;
            }
        } else if piece < WHITE_LIGHT {
            if (self.tables.n_att[to] & king) != 0 {
                return true;
            }
        } else if piece < WHITE_ROOK {
            if (self.tables.b_mask[to] & king) != 0
                && (self.tables.between[king_sq][to] & self.piece_all()) == 0
            {
                return true;
            }
        } else if piece < WHITE_QUEEN {
            if (self.tables.r_mask[to] & king) != 0
                && (self.tables.between[king_sq][to] & self.piece_all()) == 0
            {
                return true;
            }
        } else if piece < WHITE_KING {
            if (self.tables.q_mask[to] & king) != 0
                && (self.tables.between[king_sq][to] & self.piece_all()) == 0
            {
                return true;
            }
        }
        false
    }

    /// Direct attack test used by the legal-move helper and tests.
    pub fn is_square_attacked(&self, s: Square, by: Color) -> bool {
        let occ = self.piece_all();
        if (self.tables.p_att[opp(by)][s] & self.pawn(by)) != 0 {
            return true;
        }
        if (self.tables.n_att[s] & self.knight(by)) != 0 {
            return true;
        }
        if (self.tables.s_area[s] & self.king(by)) != 0 {
            return true;
        }
        if (self.tables.bishop_attacks(s, occ) & self.bslider(by)) != 0 {
            return true;
        }
        if (self.tables.rook_attacks(s, occ) & self.rslider(by)) != 0 {
            return true;
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Move generation
// ---------------------------------------------------------------------------

impl Engine {
    #[inline(always)]
    fn add_move(list: &mut [i32], count: &mut usize, mv: i32) {
        list[*count] = mv;
        *count += 1;
    }

    #[inline(always)]
    fn add_move_score(
        list: &mut [i32],
        count: &mut usize,
        from: Square,
        to: Square,
        flags: i32,
        score: i32,
    ) {
        list[*count] = ((from as i32) << 6) | (to as i32) | flags | score;
        *count += 1;
    }

    #[inline(always)]

    /// `gen_captures<me>`: returns the end index (exclusive of the 0
    /// terminator).
    pub fn gen_captures(&mut self, me: Color) -> usize {
        let t = self.tables;
        let cur = self.pos.cur;
        let st = self.pos.data[cur];
        let list =
            unsafe { std::slice::from_raw_parts_mut(self.move_lists[cur].as_mut_ptr(), MAX_MOVES) };
        let mut n = 0usize;
        let oppc = opp(me);
        let mask = st.mask;
        let mut v;
        let mut u;

        if st.ep_square != 0 {
            let mut vv = t.p_att[oppc][st.ep_square as usize] & self.pawn(me);
            while vv != 0 {
                let from = lsb(vv);
                cut(&mut vv);
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    st.ep_square as usize,
                    FLAG_EP,
                    t.mvv_lva[ipawn(me)][ipawn(oppc)],
                );
            }
        }
        let mut uu = self.pawn(me) & self.line(me, 6);
        while uu != 0 {
            let from = lsb(uu);
            cut(&mut uu);
            let to = (from as i32 + Self::push(me)) as usize;
            if self.pos.board.square[to] == 0 {
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    to,
                    FLAG_P_QUEEN,
                    t.mvv_lva[WHITE_QUEEN][BLACK_QUEEN],
                );
                if (t.n_att[lsb(self.king(oppc))] & bit(to)) != 0 {
                    Self::add_move_score(
                        list,
                        &mut n,
                        from,
                        to,
                        FLAG_P_KNIGHT,
                        t.mvv_lva[WHITE_KNIGHT][BLACK_KNIGHT],
                    );
                }
            }
        }
        v = Self::shift_w(oppc, mask) & self.pawn(me) & self.line(me, 6);
        while v != 0 {
            let from = lsb(v);
            cut(&mut v);
            let to = (from as i32 + Self::push_e(me)) as usize;
            let cap = self.pos.board.square[to] as usize;
            let score = t.mvv_lva[promotion_capture_piece(cap)][BLACK_QUEEN];
            Self::add_move_score(list, &mut n, from, to, FLAG_P_QUEEN, score);
            if (t.n_att[lsb(self.king(oppc))] & bit(to)) != 0 {
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    to,
                    FLAG_P_KNIGHT,
                    t.mvv_lva[WHITE_KING][cap],
                );
            }
        }
        v = Self::shift_e(oppc, mask) & self.pawn(me) & self.line(me, 6);
        while v != 0 {
            let from = lsb(v);
            cut(&mut v);
            let to = (from as i32 + Self::push_w(me)) as usize;
            let cap = self.pos.board.square[to] as usize;
            let score = t.mvv_lva[promotion_capture_piece(cap)][BLACK_QUEEN];
            Self::add_move_score(list, &mut n, from, to, FLAG_P_QUEEN, score);
            if (t.n_att[lsb(self.king(oppc))] & bit(to)) != 0 {
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    to,
                    FLAG_P_KNIGHT,
                    t.mvv_lva[WHITE_KING][cap],
                );
            }
        }
        if (st.att[me] & mask) == 0 {
            list[n] = 0;
            return n;
        }
        v = Self::shift_w(oppc, mask) & self.pawn(me) & !self.line(me, 6);
        while v != 0 {
            let from = lsb(v);
            cut(&mut v);
            Self::add_move_score(
                list,
                &mut n,
                from,
                (from as i32 + Self::push_e(me)) as usize,
                0,
                t.mvv_lva[ipawn(me)]
                    [self.pos.board.square[(from as i32 + Self::push_e(me)) as usize] as usize],
            );
        }
        v = Self::shift_e(oppc, mask) & self.pawn(me) & !self.line(me, 6);
        while v != 0 {
            let from = lsb(v);
            cut(&mut v);
            Self::add_move_score(
                list,
                &mut n,
                from,
                (from as i32 + Self::push_w(me)) as usize,
                0,
                t.mvv_lva[ipawn(me)]
                    [self.pos.board.square[(from as i32 + Self::push_w(me)) as usize] as usize],
            );
        }
        v = t.s_area[lsb(self.king(me))] & mask & !st.att[oppc];
        while v != 0 {
            let to = lsb(v);
            cut(&mut v);
            Self::add_move_score(
                list,
                &mut n,
                lsb(self.king(me)),
                to,
                0,
                t.mvv_lva[iking(me)][self.pos.board.square[to] as usize],
            );
        }
        u = self.knight(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            v = t.n_att[from] & mask;
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    to,
                    0,
                    t.mvv_lva[iknight(me)][self.pos.board.square[to] as usize],
                );
            }
        }
        u = self.bishop(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            v = t.bishop_attacks(from, self.piece_all()) & mask;
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    to,
                    0,
                    t.mvv_lva[self.pos.board.square[from] as usize]
                        [self.pos.board.square[to] as usize],
                );
            }
        }
        u = self.rook(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            v = t.rook_attacks(from, self.piece_all()) & mask;
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    to,
                    0,
                    t.mvv_lva[irook(me)][self.pos.board.square[to] as usize],
                );
            }
        }
        u = self.queen(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            v = t.queen_attacks(from, self.piece_all()) & mask;
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    to,
                    0,
                    t.mvv_lva[iqueen(me)][self.pos.board.square[to] as usize],
                );
            }
        }
        list[n] = 0;
        n
    }

    /// `gen_evasions<me>`
    pub fn gen_evasions(&mut self, me: Color) -> usize {
        let t = self.tables;
        let cur = self.pos.cur;
        let st = self.pos.data[cur];
        let list =
            unsafe { std::slice::from_raw_parts_mut(self.move_lists[cur].as_mut_ptr(), MAX_MOVES) };
        let mut n = 0usize;
        let oppc = opp(me);
        let king = lsb(self.king(me));
        let mut att = (t.n_att[king] & self.knight(oppc)) | (t.p_att[me][king] & self.pawn(oppc));
        let mut u = (t.b_mask[king] & self.bslider(oppc)) | (t.r_mask[king] & self.rslider(oppc));
        while u != 0 {
            let s = lsb(u);
            u ^= bit(s);
            if (t.between[king][s] & self.piece_all()) == 0 {
                att |= bit(s);
            }
        }
        if att == 0 {
            let mut sq_bb = [0u64; 16];
            for s in 0..64 {
                let p = self.pos.board.square[s] as usize;
                if p != 0 {
                    sq_bb[p] |= bit(s);
                    sq_bb[p & 1] |= bit(s);
                }
            }
            let mut bad = 0;
            for p in 0..16 {
                if sq_bb[p] != self.pos.board.bb[p] {
                    eprintln!(
                        "piece {} bb {:x} squares {:x}",
                        p, self.pos.board.bb[p], sq_bb[p]
                    );
                    bad += 1;
                }
            }
            {
                let cur = self.pos.cur;
                let st = self.pos.data[cur];
                eprintln!(
                    "gen_evasions: no checker! me={} turn={} king={} fen={} att={:x} raw_att={:x} attacked={} depth={} bad={} eval_key={:x} key={:x} flags? eval_cached={}",
                    me,
                    st.turn,
                    king,
                    self.to_fen(),
                    st.att[oppc],
                    st.att[oppc],
                    self.is_square_attacked(king, oppc),
                    self.stats.depth,
                    bad,
                    st.eval_key,
                    st.key,
                    st.eval_key == st.key
                );
                self.pos.data[cur].eval_key = 0;
                self.evaluate();
                eprintln!(
                    "forced eval att={:x} check={}",
                    self.pos.data[cur].att[oppc],
                    self.check(me)
                );
            }
            eprintln!("{}", std::backtrace::Backtrace::force_capture());
            list[n] = 0;
            return n;
        }
        let mut att_sq = lsb(att);
        let mut esc = t.s_area[king] & !(self.piece(me) | st.att[oppc]) & st.mask;
        if self.pos.board.square[att_sq] >= WHITE_LIGHT as u8 {
            esc &= !t.full_line[king][att_sq];
        }
        att &= att - 1;
        if att != 0 {
            att_sq = lsb(att);
            if self.pos.board.square[att_sq] >= WHITE_LIGHT as u8 {
                esc &= !t.full_line[king][att_sq];
            }
            while esc != 0 {
                let to = lsb(esc);
                cut(&mut esc);
                Self::add_move_score(
                    list,
                    &mut n,
                    king,
                    to,
                    0,
                    t.mvv_lva[iking(me)][self.pos.board.square[to] as usize],
                );
            }
            list[n] = 0;
            return n;
        }
        if (bit(att_sq) & st.mask) != 0 {
            let ep_sq = att_sq as i32 + Self::push(me);
            if st.ep_square != 0 && st.ep_square as i32 == ep_sq {
                let mut v = t.p_att[oppc][ep_sq as usize] & self.pawn(me);
                while v != 0 {
                    let from = lsb(v);
                    cut(&mut v);
                    Self::add_move_score(
                        list,
                        &mut n,
                        from,
                        ep_sq as usize,
                        FLAG_EP,
                        t.mvv_lva[ipawn(me)][ipawn(oppc)],
                    );
                }
            }
        }
        let mut v = t.p_att[oppc][att_sq] & self.pawn(me);
        while v != 0 {
            let from = lsb(v);
            cut(&mut v);
            if (bit(att_sq) & self.line(me, 7)) != 0 {
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    att_sq,
                    FLAG_P_QUEEN,
                    t.mvv_lva[promotion_capture_piece(self.pos.board.square[att_sq] as usize)]
                        [BLACK_QUEEN],
                );
            } else if (bit(att_sq) & st.mask) != 0 {
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    att_sq,
                    0,
                    t.mvv_lva[ipawn(me)][self.pos.board.square[att_sq] as usize],
                );
            }
        }
        while esc != 0 {
            let to = lsb(esc);
            cut(&mut esc);
            Self::add_move_score(
                list,
                &mut n,
                king,
                to,
                0,
                t.mvv_lva[iking(me)][self.pos.board.square[to] as usize],
            );
        }
        let att = t.between[king][att_sq];
        v = Self::shift(oppc, att) & self.pawn(me);
        while v != 0 {
            let from = lsb(v);
            cut(&mut v);
            if (bit(from) & self.line(me, 6)) != 0 {
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    (from as i32 + Self::push(me)) as usize,
                    FLAG_P_QUEEN,
                    t.mvv_lva[WHITE_QUEEN][BLACK_QUEEN],
                );
            } else if st.mask == !0u64 {
                // F(~Current->mask) when mask == Filled
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    (from as i32 + Self::push(me)) as usize,
                    0,
                    0,
                );
            }
        }
        if st.mask == !0u64 {
            let mut v =
                Self::shift(oppc, Self::shift(oppc, att)) & self.line(me, 1) & self.pawn(me);
            while v != 0 {
                let from = lsb(v);
                cut(&mut v);
                if self.pos.board.square[(from as i32 + Self::push(me)) as usize] == 0 {
                    Self::add_move(
                        list,
                        &mut n,
                        ((from as i32) << 6) | (from as i32 + 2 * Self::push(me)),
                    );
                }
            }
        }
        let att = att | bit(att_sq);
        let mut u = self.knight(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut e = t.n_att[from] & att;
            while e != 0 {
                let to = lsb(e);
                e ^= bit(to);
                if (bit(to) & st.mask) != 0 {
                    Self::add_move_score(
                        list,
                        &mut n,
                        from,
                        to,
                        0,
                        t.mvv_lva[iknight(me)][self.pos.board.square[to] as usize],
                    );
                }
            }
        }
        let mut u = self.bishop(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut e = t.bishop_attacks(from, self.piece_all()) & att;
            while e != 0 {
                let to = lsb(e);
                e ^= bit(to);
                if (bit(to) & st.mask) != 0 {
                    Self::add_move_score(
                        list,
                        &mut n,
                        from,
                        to,
                        0,
                        t.mvv_lva[self.pos.board.square[from] as usize]
                            [self.pos.board.square[to] as usize],
                    );
                }
            }
        }
        let mut u = self.rook(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut e = t.rook_attacks(from, self.piece_all()) & att;
            while e != 0 {
                let to = lsb(e);
                e ^= bit(to);
                if (bit(to) & st.mask) != 0 {
                    Self::add_move_score(
                        list,
                        &mut n,
                        from,
                        to,
                        0,
                        t.mvv_lva[irook(me)][self.pos.board.square[to] as usize],
                    );
                }
            }
        }
        let mut u = self.queen(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut e = t.queen_attacks(from, self.piece_all()) & att;
            while e != 0 {
                let to = lsb(e);
                e ^= bit(to);
                if (bit(to) & st.mask) != 0 {
                    Self::add_move_score(
                        list,
                        &mut n,
                        from,
                        to,
                        0,
                        t.mvv_lva[iqueen(me)][self.pos.board.square[to] as usize],
                    );
                }
            }
        }
        list[n] = 0;
        n
    }

    pub fn mark_evasions(&mut self) {
        let cur = self.pos.cur;
        let st = self.pos.data[cur];
        let list =
            unsafe { std::slice::from_raw_parts_mut(self.move_lists[cur].as_mut_ptr(), MAX_MOVES) };
        let mut i = 0;
        while list[i] != 0 {
            let mv = list[i] & 0xFFFF;
            if self.pos.board.square[move_to(mv)] == 0 && (mv & 0xE000) == 0 {
                if mv as u16 == st.refs[0] {
                    list[i] |= REF_ONE_SCORE;
                } else if mv as u16 == st.refs[1] {
                    list[i] |= REF_TWO_SCORE;
                } else if mv as u16 == st.killer[1] {
                    list[i] |= KILLER_ONE_SCORE;
                } else if mv as u16 == st.killer[2] {
                    list[i] |= KILLER_TWO_SCORE;
                } else {
                    list[i] |= self.history_p(self.pos.board.square[move_from(mv)] as usize, mv);
                }
            }
            i += 1;
        }
    }

    /// `gen_quiet_moves<me>`
    pub fn gen_quiet_moves(&mut self, me: Color, start: usize) -> usize {
        let t = self.tables;
        let cur = self.pos.cur;
        let st = self.pos.data[cur];
        let list =
            unsafe { std::slice::from_raw_parts_mut(self.move_lists[cur].as_mut_ptr(), MAX_MOVES) };
        let mut n = start;
        let occ = self.piece_all();
        let free = !occ;
        if me == WHITE {
            if (st.castle_flags & CAN_CASTLE_OO) != 0
                && (occ & 0x60) == 0
                && (st.att[BLACK] & 0x70) == 0
            {
                let score = self.history_p(iking(WHITE), ((4 << 6) | 6 | FLAG_CASTLING) as i32);
                Self::add_move_score(list, &mut n, 4, 6, FLAG_CASTLING, score);
            }
            if (st.castle_flags & CAN_CASTLE_OOO) != 0
                && (occ & 0xE) == 0
                && (st.att[BLACK] & 0x1C) == 0
            {
                let score = self.history_p(iking(WHITE), ((4 << 6) | 2 | FLAG_CASTLING) as i32);
                Self::add_move_score(list, &mut n, 4, 2, FLAG_CASTLING, score);
            }
        } else {
            if (st.castle_flags & CAN_CASTLE_oo) != 0
                && (occ & 0x6000_0000_0000_0000) == 0
                && (st.att[WHITE] & 0x7000_0000_0000_0000) == 0
            {
                let score = self.history_p(iking(BLACK), ((60 << 6) | 62 | FLAG_CASTLING) as i32);
                Self::add_move_score(list, &mut n, 60, 62, FLAG_CASTLING, score);
            }
            if (st.castle_flags & CAN_CASTLE_ooo) != 0
                && (occ & 0x0E00_0000_0000_0000) == 0
                && (st.att[WHITE] & 0x1C00_0000_0000_0000) == 0
            {
                let score = self.history_p(iking(BLACK), ((60 << 6) | 58 | FLAG_CASTLING) as i32);
                Self::add_move_score(list, &mut n, 60, 58, FLAG_CASTLING, score);
            }
        }
        let mut v = Self::shift(me, self.pawn(me)) & free & !self.line(me, 7);
        while v != 0 {
            let to = lsb(v);
            cut(&mut v);
            if (bit(to) & self.line(me, 2)) != 0
                && self.pos.board.square[(to as i32 + Self::push(me)) as usize] == 0
            {
                let from = (to as i32 - Self::push(me)) as usize;
                let final_to = (to as i32 + Self::push(me)) as usize;
                let score =
                    self.history_p(ipawn(me), (((from as i32) << 6) | final_to as i32) as i32);
                Self::add_move_score(list, &mut n, from, final_to, 0, score);
            }
            let from = (to as i32 - Self::push(me)) as usize;
            let score = self.history_p(ipawn(me), (((from as i32) << 6) | to as i32) as i32);
            Self::add_move_score(list, &mut n, from, to, 0, score);
        }
        let mut u = self.knight(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut v = free & t.n_att[from];
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                let score = self.history_p(iknight(me), (((from as i32) << 6) | to as i32) as i32);
                Self::add_move_score(list, &mut n, from, to, 0, score);
            }
        }
        let mut u = self.bishop(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut v = free & t.bishop_attacks(from, occ);
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                let score = self.history_p(
                    self.pos.board.square[from] as usize,
                    (((from as i32) << 6) | to as i32) as i32,
                );
                Self::add_move_score(list, &mut n, from, to, 0, score);
            }
        }
        let mut u = self.rook(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut v = free & t.rook_attacks(from, occ);
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                let score = self.history_p(irook(me), (((from as i32) << 6) | to as i32) as i32);
                Self::add_move_score(list, &mut n, from, to, 0, score);
            }
        }
        let mut u = self.queen(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut v = free & t.queen_attacks(from, occ);
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                let score = self.history_p(iqueen(me), (((from as i32) << 6) | to as i32) as i32);
                Self::add_move_score(list, &mut n, from, to, 0, score);
            }
        }
        let mut v = t.s_area[lsb(self.king(me))] & free & !st.att[opp(me)];
        while v != 0 {
            let to = lsb(v);
            cut(&mut v);
            let from = lsb(self.king(me));
            let score = self.history_p(iking(me), (((from as i32) << 6) | to as i32) as i32);
            Self::add_move_score(list, &mut n, from, to, 0, score);
        }
        list[n] = 0;
        n
    }

    /// `gen_checks<me>`
    pub fn gen_checks(&mut self, me: Color) -> usize {
        let t = self.tables;
        let cur = self.pos.cur;
        let st = self.pos.data[cur];
        let list =
            unsafe { std::slice::from_raw_parts_mut(self.move_lists[cur].as_mut_ptr(), MAX_MOVES) };
        let mut n = 0usize;
        let oppc = opp(me);
        let clear = !(self.piece(me) | st.mask);
        let king = lsb(self.king(oppc));
        let mut u = st.xray[me] & self.piece(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let target = clear & !t.full_line[king][from];
            if self.pos.board.square[from] as usize == ipawn(me) {
                if (bit((from as i32 + Self::push(me)) as usize) & self.line(me, 7)) == 0 {
                    let to = (from as i32 + Self::push(me)) as usize;
                    if (bit(to) & target) != 0 && self.pos.board.square[to] == 0 {
                        Self::add_move_score(
                            list,
                            &mut n,
                            from,
                            to,
                            0,
                            t.mvv_lva[WHITE_QUEEN][WHITE_PAWN],
                        );
                    }
                    let mut v = t.p_att[me][from] & target & self.piece(oppc);
                    while v != 0 {
                        let to = lsb(v);
                        cut(&mut v);
                        Self::add_move_score(
                            list,
                            &mut n,
                            from,
                            to,
                            0,
                            t.mvv_lva[WHITE_KING][self.pos.board.square[to] as usize],
                        );
                    }
                }
            } else {
                let piece = self.pos.board.square[from] as usize;
                let mut v = if piece < WHITE_LIGHT {
                    t.n_att[from] & target
                } else if piece < WHITE_ROOK {
                    t.bishop_attacks(from, self.piece_all()) & target
                } else if piece < WHITE_QUEEN {
                    t.rook_attacks(from, self.piece_all()) & target
                } else if piece < WHITE_KING {
                    t.queen_attacks(from, self.piece_all()) & target
                } else {
                    t.s_area[from] & target & !st.att[oppc]
                };
                while v != 0 {
                    let to = lsb(v);
                    cut(&mut v);
                    Self::add_move_score(
                        list,
                        &mut n,
                        from,
                        to,
                        0,
                        t.mvv_lva[WHITE_KING][self.pos.board.square[to] as usize],
                    );
                }
            }
        }
        let xray = !(st.xray[me] & self.piece(me));
        let mut u = self.knight(me) & t.n_area[king] & xray;
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut v = t.n_att[king] & t.n_att[from] & clear;
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    to,
                    0,
                    t.mvv_lva[iknight(me)][self.pos.board.square[to] as usize],
                );
            }
        }
        let mut u = t.d_area[king] & self.pawn(me) & !self.line(me, 6) & xray;
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut v = t.p_att[me][from] & t.p_att[oppc][king] & clear & self.piece(oppc);
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    to,
                    0,
                    t.mvv_lva[ipawn(me)][self.pos.board.square[to] as usize],
                );
            }
            let to = (from as i32 + Self::push(me)) as usize;
            if self.pos.board.square[to] == 0 && (bit(to) & t.p_att[oppc][king]) != 0 {
                Self::add_move(list, &mut n, ((from as i32) << 6) | to as i32);
            }
        }
        let b_target = t.bishop_attacks(king, self.piece_all()) & clear;
        let r_target = t.rook_attacks(king, self.piece_all()) & clear;
        let bishop_bb = if (king ^ rank(king)) & 1 != 0 {
            self.pos.board.bb[WHITE_LIGHT | me]
        } else {
            self.pos.board.bb[WHITE_DARK | me]
        } & xray;
        let mut u = bishop_bb;
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut v = t.bishop_attacks(from, self.piece_all()) & b_target;
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    to,
                    0,
                    t.mvv_lva[self.pos.board.square[from] as usize]
                        [self.pos.board.square[to] as usize],
                );
            }
        }
        let mut u = self.rook(me) & xray;
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut v = t.rook_attacks(from, self.piece_all()) & r_target;
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    to,
                    0,
                    t.mvv_lva[irook(me)][self.pos.board.square[to] as usize],
                );
            }
        }
        let mut u = self.queen(me) & xray;
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut v = t.queen_attacks(from, self.piece_all()) & (b_target | r_target);
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                Self::add_move_score(
                    list,
                    &mut n,
                    from,
                    to,
                    0,
                    t.mvv_lva[iqueen(me)][self.pos.board.square[to] as usize],
                );
            }
        }
        list[n] = 0;
        n
    }

    /// `gen_delta_moves<me>`
    pub fn gen_delta_moves(&mut self, me: Color) -> usize {
        let t = self.tables;
        let cur = self.pos.cur;
        let st = self.pos.data[cur];
        let list =
            unsafe { std::slice::from_raw_parts_mut(self.move_lists[cur].as_mut_ptr(), MAX_MOVES) };
        let mut n = 0usize;
        let oppc = opp(me);
        let occ = self.piece_all();
        let free = !occ;
        if me == WHITE {
            if (st.castle_flags & CAN_CASTLE_OO) != 0
                && (occ & 0x60) == 0
                && (st.att[BLACK] & 0x70) == 0
            {
                Self::add_delta(
                    list,
                    &mut n,
                    &self.delta,
                    iking(WHITE),
                    4,
                    6,
                    FLAG_CASTLING,
                    st.margin,
                );
            }
            if (st.castle_flags & CAN_CASTLE_OOO) != 0
                && (occ & 0xE) == 0
                && (st.att[BLACK] & 0x1C) == 0
            {
                Self::add_delta(
                    list,
                    &mut n,
                    &self.delta,
                    iking(WHITE),
                    4,
                    2,
                    FLAG_CASTLING,
                    st.margin,
                );
            }
        } else {
            if (st.castle_flags & CAN_CASTLE_oo) != 0
                && (occ & 0x6000_0000_0000_0000) == 0
                && (st.att[WHITE] & 0x7000_0000_0000_0000) == 0
            {
                Self::add_delta(
                    list,
                    &mut n,
                    &self.delta,
                    iking(BLACK),
                    60,
                    62,
                    FLAG_CASTLING,
                    st.margin,
                );
            }
            if (st.castle_flags & CAN_CASTLE_ooo) != 0
                && (occ & 0x0E00_0000_0000_0000) == 0
                && (st.att[WHITE] & 0x1C00_0000_0000_0000) == 0
            {
                Self::add_delta(
                    list,
                    &mut n,
                    &self.delta,
                    iking(BLACK),
                    60,
                    58,
                    FLAG_CASTLING,
                    st.margin,
                );
            }
        }
        let mut v = Self::shift(me, self.pawn(me)) & free & !self.line(me, 7);
        while v != 0 {
            let to = lsb(v);
            cut(&mut v);
            if (bit(to) & self.line(me, 2)) != 0
                && self.pos.board.square[(to as i32 + Self::push(me)) as usize] == 0
            {
                let from = (to as i32 - Self::push(me)) as usize;
                Self::add_delta(
                    list,
                    &mut n,
                    &self.delta,
                    ipawn(me),
                    from,
                    (to as i32 + Self::push(me)) as usize,
                    0,
                    st.margin,
                );
            }
            let from = (to as i32 - Self::push(me)) as usize;
            Self::add_delta(list, &mut n, &self.delta, ipawn(me), from, to, 0, st.margin);
        }
        let mut u = self.knight(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut v = free & t.n_att[from];
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                Self::add_delta(
                    list,
                    &mut n,
                    &self.delta,
                    iknight(me),
                    from,
                    to,
                    0,
                    st.margin,
                );
            }
        }
        let mut u = self.bishop(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut v = free & t.bishop_attacks(from, occ);
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                Self::add_delta(
                    list,
                    &mut n,
                    &self.delta,
                    self.pos.board.square[from] as usize,
                    from,
                    to,
                    0,
                    st.margin,
                );
            }
        }
        let mut u = self.rook(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut v = free & t.rook_attacks(from, occ);
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                Self::add_delta(list, &mut n, &self.delta, irook(me), from, to, 0, st.margin);
            }
        }
        let mut u = self.queen(me);
        while u != 0 {
            let from = lsb(u);
            cut(&mut u);
            let mut v = free & t.queen_attacks(from, occ);
            while v != 0 {
                let to = lsb(v);
                cut(&mut v);
                Self::add_delta(
                    list,
                    &mut n,
                    &self.delta,
                    iqueen(me),
                    from,
                    to,
                    0,
                    st.margin,
                );
            }
        }
        let mut v = t.s_area[lsb(self.king(me))] & free & !st.att[oppc];
        while v != 0 {
            let to = lsb(v);
            cut(&mut v);
            let from = lsb(self.king(me));
            Self::add_delta(list, &mut n, &self.delta, iking(me), from, to, 0, st.margin);
        }
        list[n] = 0;
        n
    }

    #[inline]
    fn add_delta(
        list: &mut [i32],
        n: &mut usize,
        delta: &[i16; 16 * 4096],
        piece: usize,
        from: usize,
        to: usize,
        flags: i32,
        margin: i32,
    ) {
        let d = delta[((piece) << 12) | ((from) << 6) | (to)] as i32;
        if d >= margin {
            list[*n] =
                ((from as i32) << 6) | (to as i32) | flags | (((d + 0x4000) as u32) << 16) as i32;
            *n += 1;
        }
    }

    /// History score packing (`HistoryP`).
    #[inline]
    pub fn history_p(&self, piece: usize, mv: i32) -> i32 {
        let to = move_to(mv);
        let h = self.history[((piece) << 6) | to] as i32;
        let hi = h & 0xFF00;
        let lo = h & 0x00FF;
        ((hi / lo) << 16) as i32
    }

    /// Generate all legal moves for `me` (helper used by UCI and tests; the
    /// search itself uses the staged generators above).
    #[inline(always)]
    fn load_moves_list(&self, cur: usize) -> [i32; MAX_MOVES] {
        let mut list = [0i32; MAX_MOVES];
        let mut n = 0usize;
        loop {
            let mv = self.move_lists[cur][n];
            list[n] = mv;
            if mv == 0 {
                break;
            }
            n += 1;
        }
        list
    }

    pub fn gen_legal_moves(&mut self, me: Color, out: &mut [i32]) -> usize {
        let cur = self.pos.cur;
        self.pos.data[cur].mask = self.piece(opp(me));
        let n = if self.check(me) {
            self.pos.data[cur].mask = !0u64;
            self.gen_evasions(me)
        } else {
            let n = self.gen_captures(me);
            self.gen_quiet_moves(me, n)
        };
        let list = self.load_moves_list(cur);
        let mut k = 0usize;
        for &mv in list.iter().take(n) {
            if mv == 0 {
                break;
            }
            if k >= out.len() {
                break;
            }
            self.do_move(me, mv);
            let king_sq = lsb(self.king(me));
            let attacked = self.is_square_attacked(king_sq, opp(me));
            self.undo_move(me, mv);
            if !attacked {
                out[k] = mv;
                k += 1;
            }
        }
        k
    }
}

#[inline]
fn promotion_capture_piece(cap: usize) -> usize {
    if cap < WHITE_ROOK {
        WHITE_ROOK
    } else if cap >= WHITE_QUEEN {
        WHITE_KING
    } else {
        WHITE_KNIGHT
    }
}

// ---------------------------------------------------------------------------
// Move ordering
// ---------------------------------------------------------------------------

pub fn sort(start: usize, finish: usize, list: &mut [i32]) {
    if finish <= start + 1 {
        return;
    }
    for p in start..finish - 1 {
        let mut best = p;
        let mut value = list[p];
        let previous = list[p];
        for q in p + 1..finish {
            if list[q] > value {
                value = list[q];
                best = q;
            }
        }
        list[best] = previous;
        list[p] = value;
    }
}

pub fn sort_moves(start: usize, finish: usize, list: &mut [i32]) {
    if finish <= start + 1 {
        return;
    }
    for p in start + 1..finish {
        let mut q = p;
        while q > start {
            if (list[q - 1] >> 16) < (list[q] >> 16) {
                list.swap(q - 1, q);
                q -= 1;
            } else {
                break;
            }
        }
    }
}

impl Engine {
    #[inline(always)]
    pub fn pick_move(&mut self) -> i32 {
        let cur = self.pos.cur;
        let idx = self.pos.data[cur].current;
        let list = &mut self.move_lists[cur];
        let mv = list[idx];
        if mv == 0 {
            return 0;
        }
        let mut best = idx;
        let mut best_val = mv;
        let mut p = idx + 1;
        while list[p] != 0 {
            if list[p] > best_val {
                best = p;
                best_val = list[p];
            }
            p += 1;
        }
        let value = list[best];
        list[best] = list[idx];
        list[idx] = value;
        self.pos.data[cur].current = idx + 1;
        value & 0xFFFF
    }

    pub fn gen_next_moves(&mut self, me: Color) {
        let cur = self.pos.cur;
        let mut st = self.pos.data[cur];
        st.gen_flags &= !FLAG_SORT;
        self.pos.data[cur] = st;
        let list =
            unsafe { std::slice::from_raw_parts_mut(self.move_lists[cur].as_mut_ptr(), MAX_MOVES) };
        match st.stage {
            S_HASH_MOVE | R_HASH_MOVE | E_HASH_MOVE => {
                list[0] = st.killer[0] as i32;
                list[1] = 0;
            }
            S_GOOD_CAP => {
                self.pos.data[cur].mask = self.piece(opp(me));
                let r = self.gen_captures(me);
                let mut p = 0usize;
                let mut q = r as isize - 1;
                while q >= p as isize {
                    let mv = list[q as usize] & 0xFFFF;
                    if !self.see(me, mv, 0) {
                        let next = list[p];
                        list[p] = list[q as usize];
                        list[q as usize] = next;
                        p += 1;
                    } else {
                        q -= 1;
                    }
                }
                self.pos.data[cur].start = p;
                self.pos.data[cur].current = p;
                sort(p, r, &mut self.move_lists[cur]);
            }
            S_SPECIAL => {
                self.pos.data[cur].current = st.start;
                let mut p = st.start;
                if st.killer[1] != 0 {
                    list[p] = st.killer[1] as i32;
                    p += 1;
                }
                if st.killer[2] != 0 {
                    list[p] = st.killer[2] as i32;
                    p += 1;
                }
                if st.refs[0] != 0 && st.refs[0] != st.killer[1] && st.refs[0] != st.killer[2] {
                    list[p] = st.refs[0] as i32;
                    p += 1;
                }
                if st.refs[1] != 0 && st.refs[1] != st.killer[1] && st.refs[1] != st.killer[2] {
                    list[p] = st.refs[1] as i32;
                    p += 1;
                }
                list[p] = 0;
            }
            S_QUIET => {
                let start = st.start;
                self.gen_quiet_moves(me, start);
                self.pos.data[cur].current = start;
                self.pos.data[cur].gen_flags |= FLAG_SORT;
            }
            S_BAD_CAP => {
                let start = st.start;
                list[start] = 0;
                self.pos.data[cur].current = 0;
                if (st.gen_flags & FLAG_NO_BC_SORT) == 0 {
                    sort(0, start, &mut self.move_lists[cur]);
                }
            }
            R_CAP => {
                let r = self.gen_captures(me);
                self.pos.data[cur].current = 0;
                sort(0, r, &mut self.move_lists[cur]);
            }
            R_CHECKS => {
                let r = self.gen_checks(me);
                self.pos.data[cur].current = 0;
                sort(0, r, &mut self.move_lists[cur]);
            }
            E_EV => {
                self.pos.data[cur].mask = !0u64;
                let r = self.gen_evasions(me);
                self.mark_evasions();
                sort(0, r, list);
                self.pos.data[cur].current = 0;
            }
            _ => {}
        }
    }

    pub fn get_move(&mut self, me: Color, root: bool) -> i32 {
        if root {
            let cur = self.pos.cur;
            let idx = self.pos.data[cur].current;
            let mv = self.root_list[idx] & 0xFFFF;
            self.pos.data[cur].current = idx + 1;
            return mv;
        }
        loop {
            let cur = self.pos.cur;
            let idx = self.pos.data[cur].current;
            if self.move_lists[cur][idx] == 0 {
                let stage = self.pos.data[cur].stage + 1;
                self.pos.data[cur].stage = stage;
                if ((1u32 << stage) & STAGE_NONE) != 0 {
                    return 0;
                }
                self.gen_next_moves(me);
                continue;
            }
            let move_val = if (self.pos.data[cur].gen_flags & FLAG_SORT) != 0 {
                self.pick_move()
            } else {
                let mv = self.move_lists[cur][idx] & 0xFFFF;
                self.pos.data[cur].current = idx + 1;
                mv
            };
            let stage = self.pos.data[cur].stage;
            if stage == S_QUIET {
                let st = &self.pos.data[cur];
                if move_val == st.killer[1] as i32
                    || move_val == st.killer[2] as i32
                    || move_val == st.refs[0] as i32
                    || move_val == st.refs[1] as i32
                {
                    continue;
                }
            } else if stage == S_SPECIAL
                && (self.pos.board.square[move_to(move_val)] != 0 || !self.is_legal(me, move_val))
            {
                continue;
            }
            return move_val;
        }
    }

    #[allow(unused_assignments)]
    pub fn gen_root_moves(&mut self, me: Color) {
        let mut depth = -256i32;
        let mut killer = 0i32;
        if let Some(idx) = self.probe_hash() {
            let e = self.hash.load(idx);
            if e.mv != 0 && e.low_depth as i32 > depth {
                depth = e.low_depth as i32;
                killer = e.mv as i32;
            }
        }
        if let Some(idx) = self.probe_pv_hash() {
            let e = self.pv_hash.load(idx);
            if e.depth as i32 > depth && e.mv != 0 {
                depth = e.depth as i32;
                killer = e.mv as i32;
            }
        }
        let cur = self.pos.cur;
        self.pos.data[cur].killer[0] = killer as u16;
        if self.check(me) {
            self.pos.data[cur].stage = STAGE_EVASION;
        } else {
            self.pos.data[cur].stage = STAGE_SEARCH;
            let st = self.pos.data[cur];
            self.pos.data[cur].refs[0] =
                self.reference[((self.pos.board.square[move_to(st.mv as i32)] as usize) << 6)
                    | move_to(st.mv as i32)]
                .refs[0];
            self.pos.data[cur].refs[1] =
                self.reference[((self.pos.board.square[move_to(st.mv as i32)] as usize) << 6)
                    | move_to(st.mv as i32)]
                .refs[1];
        }
        self.pos.data[cur].gen_flags = 0;
        let mut root = [0i32; 256];
        let mut p = 0usize;
        self.pos.data[cur].current = 0;
        self.move_lists[cur][0] = 0;
        loop {
            let mv = self.get_move(me, false);
            if mv == 0 {
                break;
            }
            if self.is_illegal(me, mv) {
                continue;
            }
            if p > 0 && mv == killer {
                continue;
            }
            if self.search_moves != 0 {
                // SearchMoves is never enabled by the UCI layer, but keep the
                // original behaviour for completeness.
                continue;
            }
            root[p] = mv;
            p += 1;
        }
        root[p] = 0;
        self.root_list = root;
    }
}

// ---------------------------------------------------------------------------
// Static exchange evaluation
// ---------------------------------------------------------------------------

impl Engine {
    #[inline(always)]
    pub fn see(&self, me: Color, mv: Move, margin: i32) -> bool {
        let t = self.tables;
        let from = move_from(mv);
        let to = move_to(mv);
        let mut piece = SEE_VALUE[self.pos.board.square[from] as usize];
        let mut capture = SEE_VALUE[self.pos.board.square[to] as usize];
        let delta = piece - capture;
        if delta <= -margin {
            return true;
        }
        if piece == SEE_VALUE[WHITE_KING] {
            return true;
        }
        let st = &self.pos.data[self.pos.cur];
        if (st.xray[me] & bit(from)) != 0 {
            return true;
        }
        if (st.pin[me] & bit(from)) != 0 && piece <= SEE_VALUE[WHITE_DARK] {
            return true;
        }
        if piece > (SEE_VALUE[WHITE_KING] >> 1) {
            return true;
        }
        if is_ep(mv) {
            return true;
        }
        if (st.att[opp(me)] & bit(to)) == 0 {
            return true;
        }
        let mut att = t.p_att[me][to] & self.pawn(opp(me));
        if att != 0 && delta + margin > SEE_VALUE[WHITE_PAWN] {
            return false;
        }
        let clear = !bit(from);
        let mut def = t.p_att[opp(me)][to] & self.pawn(me) & clear;
        if def != 0 && delta + SEE_VALUE[WHITE_PAWN] + margin <= 0 {
            return true;
        }
        att |= t.n_att[to] & self.knight(opp(me));
        if att != 0 && delta > SEE_VALUE[WHITE_DARK] - margin {
            return false;
        }
        let mut occ = self.piece_all() & clear;
        let b_area = t.bishop_attacks(to, occ);
        let opp_bishop = self.bishop(opp(me));
        if delta > SEE_VALUE[idark(me)] - margin && (b_area & opp_bishop) != 0 {
            return false;
        }
        let my_bishop = self.bishop(me);
        let b_slider_att = t.b_mask[to] & (opp_bishop | self.queen(opp(me)));
        let r_slider_att = t.r_mask[to] & self.major(opp(me));
        let b_slider_def = t.b_mask[to] & (my_bishop | self.queen(me)) & clear;
        let r_slider_def = t.r_mask[to] & self.major(me) & clear;
        att |= b_slider_att & b_area;
        def |= t.n_att[to] & self.knight(me) & clear;
        let r_area = t.rook_attacks(to, occ);
        att |= r_slider_att & r_area;
        def |= b_slider_def & b_area;
        def |= r_slider_def & r_area;
        att |= t.s_area[to] & self.king(opp(me));
        def |= t.s_area[to] & self.king(me) & clear;
        loop {
            let mut u;
            if {
                u = att & self.pawn(opp(me));
                u != 0
            } {
                capture -= piece;
                piece = SEE_VALUE[WHITE_PAWN];
                let sq = lsb(u);
                occ ^= bit(sq);
                att ^= bit(sq);
                let mut new_att = t.full_line[to][sq] & b_slider_att & occ & !att;
                while new_att != 0 {
                    let pos = lsb(new_att);
                    cut(&mut new_att);
                    if (t.between[to][pos] & occ) == 0 {
                        att |= bit(pos);
                        break;
                    }
                }
            } else if {
                u = att & self.knight(opp(me));
                u != 0
            } {
                capture -= piece;
                piece = SEE_VALUE[WHITE_KNIGHT];
                att ^= (!(u - 1)) & u;
            } else if {
                u = att & opp_bishop;
                u != 0
            } {
                capture -= piece;
                piece = SEE_VALUE[WHITE_DARK];
                let sq = lsb(u);
                occ ^= bit(sq);
                att ^= bit(sq);
                let mut new_att = t.full_line[to][sq] & b_slider_att & occ & !att;
                while new_att != 0 {
                    let pos = lsb(new_att);
                    cut(&mut new_att);
                    if (t.between[to][pos] & occ) == 0 {
                        att |= bit(pos);
                        break;
                    }
                }
            } else if {
                u = att & self.rook(opp(me));
                u != 0
            } {
                capture -= piece;
                piece = SEE_VALUE[WHITE_ROOK];
                let sq = lsb(u);
                occ ^= bit(sq);
                att ^= bit(sq);
                let mut new_att = t.full_line[to][sq] & r_slider_att & occ & !att;
                while new_att != 0 {
                    let pos = lsb(new_att);
                    cut(&mut new_att);
                    if (t.between[to][pos] & occ) == 0 {
                        att |= bit(pos);
                        break;
                    }
                }
            } else if {
                u = att & self.queen(opp(me));
                u != 0
            } {
                capture -= piece;
                piece = SEE_VALUE[WHITE_QUEEN];
                let sq = lsb(u);
                occ ^= bit(sq);
                att ^= bit(sq);
                let mut new_att = t.full_line[to][sq] & (r_slider_att | b_slider_att) & occ & !att;
                while new_att != 0 {
                    let pos = lsb(new_att);
                    cut(&mut new_att);
                    if (t.between[to][pos] & occ) == 0 {
                        att |= bit(pos);
                        break;
                    }
                }
            } else if {
                u = att & self.king(opp(me));
                u != 0
            } {
                capture -= piece;
                piece = SEE_VALUE[WHITE_KING];
            } else {
                return true;
            }
            if capture < -(SEE_VALUE[WHITE_KING] >> 1) {
                return false;
            }
            if piece + capture < margin {
                return false;
            }
            if {
                u = def & self.pawn(me);
                u != 0
            } {
                capture += piece;
                piece = SEE_VALUE[WHITE_PAWN];
                let sq = lsb(u);
                occ ^= bit(sq);
                def ^= bit(sq);
                let mut new_att = t.full_line[to][sq] & b_slider_def & occ & !att;
                while new_att != 0 {
                    let pos = lsb(new_att);
                    cut(&mut new_att);
                    if (t.between[to][pos] & occ) == 0 {
                        def |= bit(pos);
                        break;
                    }
                }
            } else if {
                u = def & self.knight(me);
                u != 0
            } {
                capture += piece;
                piece = SEE_VALUE[WHITE_KNIGHT];
                def ^= (!(u - 1)) & u;
            } else if {
                u = def & my_bishop;
                u != 0
            } {
                capture += piece;
                piece = SEE_VALUE[WHITE_DARK];
                let sq = lsb(u);
                occ ^= bit(sq);
                def ^= bit(sq);
                let mut new_att = t.full_line[to][sq] & b_slider_def & occ & !att;
                while new_att != 0 {
                    let pos = lsb(new_att);
                    cut(&mut new_att);
                    if (t.between[to][pos] & occ) == 0 {
                        def |= bit(pos);
                        break;
                    }
                }
            } else if {
                u = def & self.rook(me);
                u != 0
            } {
                capture += piece;
                piece = SEE_VALUE[WHITE_ROOK];
                let sq = lsb(u);
                occ ^= bit(sq);
                def ^= bit(sq);
                let mut new_att = t.full_line[to][sq] & r_slider_def & occ & !att;
                while new_att != 0 {
                    let pos = lsb(new_att);
                    cut(&mut new_att);
                    if (t.between[to][pos] & occ) == 0 {
                        def |= bit(pos);
                        break;
                    }
                }
            } else if {
                u = def & self.queen(me);
                u != 0
            } {
                capture += piece;
                piece = SEE_VALUE[WHITE_QUEEN];
                let sq = lsb(u);
                occ ^= bit(sq);
                def ^= bit(sq);
                let mut new_att = t.full_line[to][sq] & (r_slider_def | b_slider_def) & occ & !att;
                while new_att != 0 {
                    let pos = lsb(new_att);
                    cut(&mut new_att);
                    if (t.between[to][pos] & occ) == 0 {
                        def |= bit(pos);
                        break;
                    }
                }
            } else if {
                u = def & self.king(me);
                u != 0
            } {
                capture += piece;
                piece = SEE_VALUE[WHITE_KING];
            } else {
                return false;
            }
            if capture > (SEE_VALUE[WHITE_KING] >> 1) {
                return true;
            }
            if capture - piece >= margin {
                return true;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Hash table access
// ---------------------------------------------------------------------------

impl Engine {
    #[inline(always)]
    pub fn probe_hash(&mut self) -> Option<usize> {
        let key = self.pos.data[self.pos.cur].key;
        let idx = (hash_key_high(key) as usize) & self.hash_mask;
        let date = self.shared.date;
        for i in 0..4 {
            let mut e = self.hash.load(idx + i);
            if e.key == hash_key_low(key) {
                e.date = date;
                self.hash.store(idx + i, e);
                return Some(idx + i);
            }
        }
        None
    }

    #[inline(always)]
    pub fn probe_pv_hash(&mut self) -> Option<usize> {
        let key = self.pos.data[self.pos.cur].key;
        let idx = (hash_key_high(key) as usize) & self.pv_hash_mask;
        let date = self.shared.date;
        for i in 0..4 {
            let mut e = self.pv_hash.load(idx + i);
            if e.key == hash_key_low(key) {
                e.date = date;
                self.pv_hash.store(idx + i, e);
                return Some(idx + i);
            }
        }
        None
    }

    #[inline(always)]
    pub fn hash_high(&mut self, value: i32, depth: i32) {
        let key = self.pos.data[self.pos.cur].key;
        let base = (hash_key_high(key) as usize) & self.hash_mask;
        let mut min_score = 0x7000_0000i32;
        let mut best = base;
        for i in 0..4 {
            let idx = base + i;
            let mut e = self.hash.load(idx);
            if e.key == hash_key_low(key) {
                e.date = self.shared.date;
                if depth > e.high_depth as i32
                    || (depth == e.high_depth as i32 && (value as i16) < e.high)
                {
                    if e.low <= value as i16 {
                        e.high_depth = depth as u8;
                        e.high = value as i16;
                    } else if (e.low_depth as i32) < depth {
                        e.high_depth = depth as u8;
                        e.high = value as i16;
                        e.low = value as i16;
                    }
                }
                self.hash.store(idx, e);
                return;
            }
            let score = ((e.date as i32) << 3) + (e.high_depth.max(e.low_depth) as i32);
            if score < min_score {
                min_score = score;
                best = idx;
            }
        }
        let mut e = self.hash.load(best);
        e.date = self.shared.date;
        e.key = hash_key_low(key);
        e.high = value as i16;
        e.high_depth = depth as u8;
        e.low = 0;
        e.low_depth = 0;
        e.mv = 0;
        e.flags = 0;
        self.hash.store(best, e);
    }

    #[inline(always)]
    pub fn hash_low(&mut self, mv: i32, value: i32, depth: i32) {
        let key = self.pos.data[self.pos.cur].key;
        let base = (hash_key_high(key) as usize) & self.hash_mask;
        let mv = (mv & 0xFFFF) as u16;
        let mut min_score = 0x7000_0000i32;
        let mut best = base;
        for i in 0..4 {
            let idx = base + i;
            let mut e = self.hash.load(idx);
            if e.key == hash_key_low(key) {
                e.date = self.shared.date;
                if depth > e.low_depth as i32
                    || (depth == e.low_depth as i32 && (value as i16) > e.low)
                {
                    if mv != 0 {
                        e.mv = mv;
                    }
                    if e.high >= value as i16 {
                        e.low_depth = depth as u8;
                        e.low = value as i16;
                    } else if (e.high_depth as i32) < depth {
                        e.low_depth = depth as u8;
                        e.low = value as i16;
                        e.high = value as i16;
                    }
                } else if e.mv == 0 {
                    e.mv = mv;
                }
                self.hash.store(idx, e);
                return;
            }
            let score = ((e.date as i32) << 3) + (e.high_depth.max(e.low_depth) as i32);
            if score < min_score {
                min_score = score;
                best = idx;
            }
        }
        let mut e = self.hash.load(best);
        e.date = self.shared.date;
        e.key = hash_key_low(key);
        e.high = 0;
        e.high_depth = 0;
        e.low = value as i16;
        e.low_depth = depth as u8;
        e.mv = mv;
        e.flags = 0;
        self.hash.store(best, e);
    }

    pub fn hash_exact(
        &mut self,
        mv: i32,
        value: i32,
        depth: i32,
        exclusion: i32,
        ex_depth: i32,
        knodes: i32,
    ) {
        let key = self.pos.data[self.pos.cur].key;
        let base = (hash_key_high(key) as usize) & self.pv_hash_mask;
        let mut min_score = 0x7000_0000i32;
        let mut best = base;
        for i in 0..4 {
            let idx = base + i;
            let mut e = self.pv_hash.load(idx);
            if e.key == hash_key_low(key) {
                e.date = self.shared.date;
                e.knodes += knodes;
                if (e.depth as i32) <= depth {
                    e.value = value as i16;
                    e.depth = depth as u8;
                    e.mv = (mv & 0xFFFF) as u16;
                    e.ply = self.pos.data[self.pos.cur].ply as i32;
                    if ex_depth != 0 {
                        e.exclusion = exclusion as i16;
                        e.ex_depth = ex_depth as u8;
                    }
                }
                self.pv_hash.store(idx, e);
                return;
            }
            let score = ((e.date as i32) << 3) + e.depth as i32;
            if score < min_score {
                min_score = score;
                best = idx;
            }
        }
        let mut e = self.pv_hash.load(best);
        e.key = hash_key_low(key);
        e.date = self.shared.date;
        e.value = value as i16;
        e.depth = depth as u8;
        e.mv = (mv & 0xFFFF) as u16;
        e.exclusion = exclusion as i16;
        e.ex_depth = ex_depth as u8;
        e.knodes = knodes;
        e.ply = self.pos.data[self.pos.cur].ply as i32;
        self.pv_hash.store(best, e);
    }
}

// ---------------------------------------------------------------------------
// Search helpers
// ---------------------------------------------------------------------------

impl Engine {
    /// `halt_check` macro.  Returns `Some(score)` when the search must unwind.
    #[inline]
    pub fn halt_check(&mut self) -> Option<i32> {
        let cur = self.pos.cur;
        if cur >= 126 {
            self.evaluate();
            return Some(self.pos.data[self.pos.cur].score as i32);
        }
        let st = self.pos.data[cur];
        if st.ply >= 100 {
            return Some(0);
        }
        let mut i = 4usize;
        while i <= st.ply as usize {
            if self.pos.stack[self.pos.sp - i] == st.key {
                return Some(0);
            }
            i += 2;
        }
        None
    }

    /// `check_for_stop`: raise a `SearchAborted` unwind when a stop was
    /// requested externally or a hard limit has been reached.
    #[cold]
    fn abort_search() -> ! {
        resume_unwind(Box::new(SearchAborted));
    }

    pub fn check_for_stop(&mut self) {
        // Polling on every node is expensive.  A 1024-node interval is still
        // well under a millisecond at current search speeds, so `stop` remains
        // responsive while the hot path only increments the node counter.
        if self.pos.nodes & 0x3FF != 0 {
            return;
        }
        if self.control.is_stopped() {
            Self::abort_search();
        }
        if self.shared.node_limit != 0 && self.pos.nodes >= self.shared.node_limit {
            Self::abort_search();
        }
        let hard = self.shared.hard_time_limit;
        if hard != INFINITE_TIME && now_ms() >= self.shared.start_time.saturating_add(hard) {
            Self::abort_search();
        }
    }

    /// `IsRepetition(margin, move)` macro.
    #[inline]
    pub fn is_repetition(&self, margin: i32, mv: Move) -> bool {
        let cur = self.pos.cur;
        let st = self.pos.data[cur];
        margin > 0
            && st.ply >= 2
            && self.pos.data[cur - 1].mv as i32
                == ((move_to(mv) as i32) << 6) | move_from(mv) as i32
            && self.pos.board.square[move_to(mv)] == 0
            && (mv & 0xF000) == 0
    }

    /// `extension<pv>(move, depth)`.
    #[inline]
    pub fn extension(&self, pv: bool, mv: Move, depth: i32) -> i32 {
        let from = move_from(mv);
        let st = self.pos.data[self.pos.cur];
        if (st.passer & bit(from)) != 0 && crank(st.turn as Color, from) >= 5 && depth < 16 {
            if pv {
                2
            } else {
                1
            }
        } else {
            0
        }
    }

    /// `capture_margin<me>`: restrict the capture mask when far below alpha.
    pub fn capture_margin(&mut self, me: Color, alpha: i32, score: &mut i32) {
        let cur = self.pos.cur;
        let st = self.pos.data[cur];
        let s = st.score as i32;
        if s + 200 < alpha {
            if (st.att[me] & self.pawn(opp(me))) != 0 {
                self.pos.data[cur].mask ^= self.pawn(opp(me));
                *score = s + 200;
            }
            if s + 500 < alpha {
                if (st.att[me] & self.minor(opp(me))) != 0 {
                    self.pos.data[cur].mask ^= self.minor(opp(me));
                    *score = s + 500;
                }
                if s + 700 < alpha {
                    if (st.att[me] & self.rook(opp(me))) != 0 {
                        self.pos.data[cur].mask ^= self.rook(opp(me));
                        *score = s + 700;
                    }
                    if s + 1400 < alpha && (st.att[me] & self.queen(opp(me))) != 0 {
                        self.pos.data[cur].mask ^= self.queen(opp(me));
                        *score = s + 1400;
                    }
                }
            }
        }
    }

    /// `singular_extension<me>`
    #[allow(clippy::too_many_arguments)]
    pub fn singular_extension(
        &mut self,
        me: Color,
        ext: i32,
        prev_ext: i32,
        margin_one: i32,
        margin_two: i32,
        depth: i32,
        killer: i32,
    ) -> i32 {
        let mut value = -MATE_VALUE;
        let mut singular = 0;
        if ext < 1 + i32::from(prev_ext < 1) {
            value = if self.check(me) {
                self.search_evasion(me, true, margin_one, depth, killer)
            } else {
                self.search(me, true, margin_one, depth, killer)
            };
            if value < margin_one {
                singular = 1;
            }
        }
        if value < margin_one && ext < 2 + i32::from(prev_ext < 1) - i32::from(prev_ext >= 2) {
            value = if self.check(me) {
                self.search_evasion(me, true, margin_two, depth, killer)
            } else {
                self.search(me, true, margin_two, depth, killer)
            };
            if value < margin_two {
                singular = 2;
            }
        }
        singular
    }

    /// `pick_pv`: fill `pv[pv_ptr..]` from the hash tables.
    #[allow(unused_assignments)]
    pub fn pick_pv(&mut self, pv: &mut [i32], pv_ptr: usize, pv_len: usize) {
        if pv_ptr >= pv_len.min(MAX_PV_LEN - 1) {
            pv[pv_ptr] = 0;
            return;
        }
        let mut depth = -256;
        let mut mv = 0;
        if let Some(idx) = self.probe_hash() {
            let e = self.hash.load(idx);
            if e.mv != 0 && (e.low_depth as i32) > depth {
                depth = e.low_depth as i32;
                mv = e.mv as i32;
            }
        }
        if let Some(idx) = self.probe_pv_hash() {
            let e = self.pv_hash.load(idx);
            if e.mv != 0 && (e.depth as i32) > depth {
                depth = e.depth as i32;
                mv = e.mv as i32;
            }
        }
        let me = self.pos.data[self.pos.cur].turn as Color;
        if mv != 0 && !self.is_illegal(me, mv) && self.is_legal(me, mv) {
            pv[pv_ptr] = mv;
            self.do_move(me, mv);
            let cur = self.pos.cur;
            let st = self.pos.data[cur];
            let mut cycle = st.ply >= 100;
            if !cycle {
                let mut i = 4usize;
                while i <= st.ply as usize {
                    if self.pos.stack[self.pos.sp - i] == st.key {
                        cycle = true;
                        break;
                    }
                    i += 2;
                }
            }
            if cycle {
                pv[pv_ptr + 1] = 0;
            } else {
                self.pick_pv(pv, pv_ptr + 1, pv_len);
            }
            let undo_me = (self.pos.data[self.pos.cur].turn ^ 1) as Color;
            self.undo_move(undo_me, mv);
        } else {
            pv[pv_ptr] = 0;
        }
    }

    /// `draw_in_pv<me>`
    pub fn draw_in_pv(&mut self, me: Color) -> bool {
        let cur = self.pos.cur;
        if cur >= 126 || self.pos.data[cur].ply >= 100 {
            return true;
        }
        let st = self.pos.data[cur];
        let mut i = 4usize;
        while i <= st.ply as usize {
            if self.pos.stack[self.pos.sp - i] == st.key {
                return true;
            }
            i += 2;
        }
        if let Some(idx) = self.probe_pv_hash() {
            let e = self.pv_hash.load(idx);
            if e.value == 0 {
                return true;
            }
            let mv = e.mv as i32;
            if mv != 0 {
                self.do_move(me, mv);
                let value = self.draw_in_pv(opp(me));
                self.undo_move(me, mv);
                return value;
            }
        }
        false
    }

    /// `send_pv` (single PV).
    pub fn send_pv(&mut self, depth: i32, _alpha: i32, _beta: i32, _score: i32) {
        if self.settings.multi_pv > 1 {
            return;
        }
        let mut sel_depth = 1usize;
        while sel_depth < 127 && self.pos.data[sel_depth].att[0] != 0 {
            sel_depth += 1;
        }
        sel_depth -= 1;
        let mv = if self.stats.best_move == 0 {
            self.root_list[0]
        } else {
            self.stats.best_move
        };
        self.stats.sel_depth = sel_depth as i32;
        let mut pv = [0i32; 64];
        let me = self.pos.data[self.pos.cur].turn as Color;
        if mv != 0 && !self.is_illegal(me, mv) && self.is_legal(me, mv) {
            pv[0] = mv;
            self.do_move(me, mv);
            self.pick_pv(&mut pv, 1, 64);
            let undo_me = (self.pos.data[self.pos.cur].turn ^ 1) as Color;
            self.undo_move(undo_me, mv);
        }
        self.stats.pv = pv;
        let elapsed = {
            let t = now_ms().saturating_sub(self.shared.start_time);
            t.max(1)
        };
        let nodes = self.pos.nodes;
        let info = Info {
            multipv: 0,
            depth: depth / 2,
            sel_depth: sel_depth as i32 / 2,
            score: self.stats.best_score,
            nodes,
            tb_hits: self.pos.tb_hits,
            time_ms: elapsed,
            pv: pv.iter().copied().take_while(|&m| m != 0).collect(),
        };
        self.reporter.pv(&info);
    }

    /// `send_curr_move`.
    pub fn send_curr_move(&mut self, mv: i32, cnt: i32) {
        if self.stats.id != 0 {
            return;
        }
        let now = now_ms();
        if now.saturating_sub(self.shared.start_time) <= INFO_LAG
            || now.saturating_sub(self.info_time as u64) <= INFO_DELAY
        {
            return;
        }
        self.info_time = now as i64;
        let elapsed = now.saturating_sub(self.shared.start_time);
        self.reporter.curr_move(mv, cnt, elapsed, self.pos.nodes);
    }

    /// `send_multipv`.
    pub fn send_multipv(&mut self, depth: i32, curr_number: i32) {
        if self.stats.id != 0 {
            return;
        }
        let pvn = self.settings.multi_pv;
        for j in 0..pvn {
            let m = self.multi_pv[j];
            if m == 0 {
                break;
            }
            let mv = m & 0xFFFF;
            let score = m >> 16;
            let mut pv = [0i32; 64];
            let me = self.pos.data[self.pos.cur].turn as Color;
            pv[0] = mv;
            self.do_move(me, mv);
            self.pick_pv(&mut pv, 1, 64);
            let undo_me = (self.pos.data[self.pos.cur].turn ^ 1) as Color;
            self.undo_move(undo_me, mv);
            let elapsed = {
                let t = now_ms().saturating_sub(self.shared.start_time);
                t.max(1)
            };
            let nodes = self.pos.nodes;
            let info = Info {
                multipv: j + 1,
                depth: if j as i32 <= curr_number {
                    depth
                } else {
                    depth - 1
                } / 2,
                sel_depth: self.stats.sel_depth / 2,
                score,
                nodes,
                tb_hits: self.pos.tb_hits,
                time_ms: elapsed,
                pv: pv.iter().copied().take_while(|&m| m != 0).collect(),
            };
            self.stats.pv = pv;
            self.reporter.pv(&info);
        }
    }

    /// `time_to_stop`.
    pub fn time_to_stop(&self, si: &SearchInfo, time: i64, searching: bool) -> bool {
        if self.shared.ponder {
            return false;
        }
        if time as u64 > self.shared.hard_time_limit {
            return true;
        }
        if searching {
            return false;
        }
        if 2 * time as u64 > self.shared.hard_time_limit {
            return true;
        }
        if si.bad != 0 {
            return false;
        }
        if time as u64 > self.shared.soft_time_limit {
            return true;
        }
        if si.change != 0 || si.fail_low != 0 {
            return false;
        }
        if time as u64 * 100 > self.shared.soft_time_limit * TIME_NO_CHANGE_MARGIN {
            return true;
        }
        if si.early == 0 {
            return false;
        }
        if time as u64 * 100 > self.shared.soft_time_limit * TIME_NO_PVSCO_MARGIN {
            return true;
        }
        if si.singular < 1 {
            return false;
        }
        if time as u64 * 100 > self.shared.soft_time_limit * TIME_SING_ONE_MARGIN {
            return true;
        }
        if si.singular < 2 {
            return false;
        }
        if time as u64 * 100 > self.shared.soft_time_limit * TIME_SING_TWO_MARGIN {
            return true;
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Small state accessors and history/reference updates
// ---------------------------------------------------------------------------

impl Engine {
    #[inline(always)]
    pub fn score(&self) -> i32 {
        self.pos.data[self.pos.cur].score as i32
    }

    #[inline(always)]
    fn delta_m(&self, mv: Move) -> i32 {
        let piece = self.pos.board.square[move_from(mv)] as usize;
        self.delta[(piece << 12) | (move_from(mv) << 6) | move_to(mv)] as i32
    }

    #[inline(always)]
    fn ref_index(&self, mv: Move) -> usize {
        let to = move_to(mv);
        ((self.pos.board.square[to] as usize) << 6) | to
    }

    #[inline(always)]
    fn update_ref(&mut self, ref_move: i32) {
        let cur = self.pos.cur;
        let mv = self.pos.data[cur].mv as i32;
        if mv == 0 {
            return;
        }
        let idx = self.ref_index(mv);
        if self.reference[idx].refs[0] as i32 != ref_move {
            self.reference[idx].refs[1] = self.reference[idx].refs[0];
            self.reference[idx].refs[0] = ref_move as u16;
        }
    }

    #[inline(always)]
    fn update_check_ref(&mut self, ref_move: i32) {
        let cur = self.pos.cur;
        let mv = self.pos.data[cur].mv as i32;
        if mv == 0 {
            return;
        }
        let idx = self.ref_index(mv);
        if self.reference[idx].check_refs[0] as i32 != ref_move {
            self.reference[idx].check_refs[1] = self.reference[idx].check_refs[0];
            self.reference[idx].check_refs[0] = ref_move as u16;
        }
    }

    #[inline(always)]
    fn history_inc(depth: i32) -> i32 {
        let d = depth >> 1;
        (d * d).min(64)
    }

    #[inline(always)]
    fn history_good(&mut self, mv: Move, depth: i32) {
        let piece = self.pos.board.square[move_from(mv)] as usize;
        let idx = (piece << 6) | move_to(mv);
        let inc = Self::history_inc(depth);
        let h = self.history[idx] as i32;
        self.history[idx] = if (h & 0x00FF) >= 256 - inc {
            (((h & 0xFEFE) >> 1) + ((inc << 8) | inc)) as i16
        } else {
            (h + ((inc << 8) | inc)) as i16
        };
    }

    #[inline(always)]
    fn history_bad(&mut self, mv: Move, depth: i32) {
        let piece = self.pos.board.square[move_from(mv)] as usize;
        let idx = (piece << 6) | move_to(mv);
        let inc = Self::history_inc(depth);
        let h = self.history[idx] as i32;
        self.history[idx] = if (h & 0x00FF) >= 256 - inc {
            (((h & 0xFEFE) >> 1) + inc) as i16
        } else {
            (h + inc) as i16
        };
    }

    #[inline]
    fn ext(flags: i32) -> i32 {
        (flags >> 16) & 0xF
    }

    #[inline]
    fn ext_flag(ext: i32) -> i32 {
        ext << 16
    }
}

// ---------------------------------------------------------------------------
// Quiescence search
// ---------------------------------------------------------------------------

impl Engine {
    #[allow(unused_assignments)]
    pub fn q_search(
        &mut self,
        me: Color,
        pv: bool,
        mut alpha: i32,
        beta: i32,
        depth: i32,
        flags: i32,
    ) -> i32 {
        if (flags & FLAG_HALT_CHECK) != 0 {
            if let Some(v) = self.halt_check() {
                return v;
            }
        }
        if (flags & FLAG_CALL_EVALUATION) != 0 {
            self.evaluate();
        }
        if self.check(me) {
            return self.q_evasion(me, pv, alpha, beta, depth, FLAG_HASH_CHECK);
        }
        let mut score = self.score() + 3;
        if score > alpha {
            alpha = score;
            if score >= beta {
                return score;
            }
        }

        let mut hash_move = 0;
        let mut hash_depth = 0;
        if (flags & FLAG_HASH_CHECK) != 0 {
            if let Some(idx) = self.probe_hash() {
                let e = self.hash.load(idx);
                if e.low_depth != 0 {
                    if e.low as i32 >= beta && !pv {
                        return e.low as i32;
                    }
                    if (e.low_depth as i32) > hash_depth && e.mv != 0 {
                        hash_move = e.mv as i32;
                        hash_depth = e.low_depth as i32;
                    }
                }
                if e.high_depth != 0 && (e.high as i32) <= alpha && !pv {
                    return e.high as i32;
                }
            }
        }

        self.pos.data[self.pos.cur].mask = self.piece(opp(me));
        self.capture_margin(me, alpha, &mut score);

        let mut cnt = 0;
        let mut cut_move = 0;
        'q: {
            if hash_move != 0 {
                let skip = (bit(move_to(hash_move)) & self.pos.data[self.pos.cur].mask) == 0
                    && (hash_move & 0xE000) == 0
                    && (depth < -8
                        || (self.score() + self.delta_m(hash_move) <= alpha
                            && !self.is_check(me, hash_move)));
                if !skip {
                    if self.is_legal(me, hash_move) {
                        if self.is_illegal(me, hash_move) {
                            // skip
                        } else {
                            if SEE_VALUE[self.pos.board.square[move_to(hash_move)] as usize]
                                > SEE_VALUE[self.pos.board.square[move_from(hash_move)] as usize]
                            {
                                cnt += 1;
                            }
                            self.do_move(me, hash_move);
                            let value = -self.q_search(
                                opp(me),
                                pv,
                                -beta,
                                -alpha,
                                depth - 1,
                                FLAG_NEAT_SEARCH,
                            );
                            self.undo_move(me, hash_move);
                            if value > score {
                                score = value;
                                if value > alpha {
                                    alpha = value;
                                    if value >= beta {
                                        cut_move = hash_move;
                                        break 'q;
                                    }
                                }
                            }
                            if (bit(move_to(hash_move)) & self.pos.data[self.pos.cur].mask) == 0
                                && (hash_move & 0xE000) == 0
                                && (depth < -2 || (depth <= -1 && self.score() + 50 < alpha))
                                && alpha >= beta - 1
                                && !pv
                            {
                                return alpha;
                            }
                        }
                    }
                }
            }
            self.gen_captures(me);
            self.pos.data[self.pos.cur].current = 0;
            loop {
                let mv = self.pick_move();
                if mv == 0 {
                    break;
                }
                if mv == hash_move {
                    continue;
                }
                if self.is_illegal(me, mv) {
                    continue;
                }
                if !self.see(me, mv, -50) {
                    continue;
                }
                if SEE_VALUE[self.pos.board.square[move_to(mv)] as usize]
                    > SEE_VALUE[self.pos.board.square[move_from(mv)] as usize]
                {
                    cnt += 1;
                }
                self.do_move(me, mv);
                let value = -self.q_search(opp(me), pv, -beta, -alpha, depth - 1, FLAG_NEAT_SEARCH);
                self.undo_move(me, mv);
                if value > score {
                    score = value;
                    if value > alpha {
                        alpha = value;
                        if value >= beta {
                            cut_move = mv;
                            break 'q;
                        }
                    }
                }
            }

            if depth < -2 {
                break 'q;
            }
            if depth <= -1 && self.score() + 50 < alpha {
                break 'q;
            }
            self.gen_checks(me);
            self.pos.data[self.pos.cur].current = 0;
            loop {
                let mv = self.pick_move();
                if mv == 0 {
                    break;
                }
                if mv == hash_move {
                    continue;
                }
                if self.is_illegal(me, mv) {
                    continue;
                }
                if self.is_repetition(alpha + 1, mv) {
                    continue;
                }
                if !self.see(me, mv, -50) {
                    continue;
                }
                self.do_move(me, mv);
                let value =
                    -self.q_evasion(opp(me), pv, -beta, -alpha, depth - 1, FLAG_NEAT_SEARCH);
                self.undo_move(me, mv);
                if value > score {
                    score = value;
                    if value > alpha {
                        alpha = value;
                        if value >= beta {
                            cut_move = mv;
                            break 'q;
                        }
                    }
                }
            }

            let st = self.pos.data[self.pos.cur];
            let oppc = opp(me);
            let delta_ok = cnt != 0
                || self.score() + 30 < alpha
                || (st.threat & self.piece(me)) != 0
                || ((st.xray[oppc] | st.pin[oppc]) & self.non_pawn(oppc)) != 0
                || (self.pawn(oppc) & self.line(me, 1) & Self::shift(me, !self.piece_all())) != 0;
            if delta_ok {
                break 'q;
            }
            self.pos.data[self.pos.cur].margin = alpha - self.score() + 6;
            self.gen_delta_moves(me);
            self.pos.data[self.pos.cur].current = 0;
            loop {
                let mv = self.pick_move();
                if mv == 0 {
                    break;
                }
                if mv == hash_move {
                    continue;
                }
                if self.is_illegal(me, mv) {
                    continue;
                }
                if self.is_repetition(alpha + 1, mv) {
                    continue;
                }
                if !self.see(me, mv, -50) {
                    continue;
                }
                cnt += 1;
                self.do_move(me, mv);
                let value = -self.q_search(opp(me), pv, -beta, -alpha, depth - 1, FLAG_NEAT_SEARCH);
                self.undo_move(me, mv);
                if value > score {
                    score = value;
                    if value > alpha {
                        alpha = value;
                        if value >= beta {
                            if self.pos.data[self.pos.cur].killer[1] as i32 != mv {
                                self.pos.data[self.pos.cur].killer[2] =
                                    self.pos.data[self.pos.cur].killer[1];
                                self.pos.data[self.pos.cur].killer[1] = mv as u16;
                            }
                            cut_move = mv;
                            break 'q;
                        }
                    }
                }
                if cnt >= 3 {
                    break;
                }
            }
        }
        if cut_move != 0 {
            self.hash_low(cut_move, score, 1);
            return score;
        }
        if depth >= -2 && (depth >= 0 || self.score() + 50 >= alpha) {
            self.hash_high(score, 1);
        }
        score
    }

    #[allow(unused_assignments)]
    pub fn q_evasion(
        &mut self,
        me: Color,
        pv: bool,
        mut alpha: i32,
        beta: i32,
        depth: i32,
        flags: i32,
    ) -> i32 {
        let mut score = self.pos.cur as i32 - MATE_VALUE;
        if (flags & FLAG_HALT_CHECK) != 0 {
            if let Some(v) = self.halt_check() {
                return v;
            }
        }
        let mut hash_move = 0;
        let mut hash_depth = 0;
        if (flags & FLAG_HASH_CHECK) != 0 {
            if let Some(idx) = self.probe_hash() {
                let e = self.hash.load(idx);
                if e.low_depth != 0 {
                    if e.low as i32 >= beta && !pv {
                        return e.low as i32;
                    }
                    if (e.low_depth as i32) > hash_depth && e.mv != 0 {
                        hash_move = e.mv as i32;
                        hash_depth = e.low_depth as i32;
                    }
                }
                if e.high_depth != 0 && (e.high as i32) <= alpha && !pv {
                    return e.high as i32;
                }
            }
        }
        if (flags & FLAG_CALL_EVALUATION) != 0 {
            self.evaluate();
        }
        self.pos.data[self.pos.cur].mask = !0u64;
        if self.score() - 10 <= alpha && !pv {
            self.pos.data[self.pos.cur].mask = self.piece(opp(me));
            score = self.score() - 10;
            self.capture_margin(me, alpha, &mut score);
        }
        alpha = alpha.max(score);
        let mut pext = 0;
        self.gen_evasions(me);
        self.pos.data[self.pos.cur].current = 0;
        if self.move_lists[self.pos.cur][0] == 0 {
            return score;
        }
        if self.move_lists[self.pos.cur][1] == 0 {
            pext = 1;
        } else {
            let cur = self.pos.cur;
            let mv = self.pos.data[cur].mv as i32;
            if mv != 0 {
                let idx = self.ref_index(mv);
                self.pos.data[cur].refs[0] = self.reference[idx].check_refs[0];
                self.pos.data[cur].refs[1] = self.reference[idx].check_refs[1];
            }
            self.mark_evasions();
            if hash_move != 0
                && (((bit(move_to(hash_move)) & self.pos.data[cur].mask) != 0)
                    || (hash_move & 0xE000) != 0)
            {
                let mut p = 0;
                while self.move_lists[cur][p] != 0 {
                    if (self.move_lists[cur][p] & 0xFFFF) == hash_move {
                        self.move_lists[cur][p] |= 0x7FFF0000;
                        break;
                    }
                    p += 1;
                }
            }
        }
        let mut cnt = 0;
        loop {
            let mv = self.pick_move();
            if mv == 0 {
                break;
            }
            if self.is_illegal(me, mv) {
                continue;
            }
            cnt += 1;
            if self.is_repetition(alpha + 1, mv) {
                score = score.max(0);
                continue;
            }
            if self.pos.board.square[move_to(mv)] == 0 && (mv & 0xE000) == 0 {
                if cnt > 3 && !self.is_check(me, mv) && !pv {
                    continue;
                }
                let value = self.score() + self.delta_m(mv) + 10;
                if value <= alpha && !pv {
                    score = value.max(score);
                    continue;
                }
            }
            self.do_move(me, mv);
            let value = -self.q_search(
                opp(me),
                pv,
                -beta,
                -alpha,
                depth - 1 + pext,
                FLAG_NEAT_SEARCH,
            );
            self.undo_move(me, mv);
            if value > score {
                score = value;
                if value > alpha {
                    alpha = value;
                    if value >= beta {
                        return score;
                    }
                }
            }
        }
        score
    }
}

// ---------------------------------------------------------------------------
// Alpha/beta search
// ---------------------------------------------------------------------------

impl Engine {
    #[allow(clippy::too_many_lines)]
    #[allow(unused_assignments)]
    pub fn search(&mut self, me: Color, exclusion: bool, beta: i32, depth: i32, flags: i32) -> i32 {
        let height = self.pos.cur as i32;
        self.check_for_stop();
        if depth <= 1 {
            return self.q_search(me, false, beta - 1, beta, 1, flags);
        }
        if (flags & FLAG_HALT_CHECK) != 0 {
            if height - MATE_VALUE >= beta {
                return beta;
            }
            if MATE_VALUE - height < beta {
                return beta - 1;
            }
            if let Some(v) = self.halt_check() {
                return v;
            }
        }

        let mut cnt = 0i32;
        let mut flag = 0i32;
        let mut singular = 0i32;
        let mut score;
        let mut hash_move;
        let mut hash_value = -MATE_VALUE;
        let mut hash_depth = -1i32;
        let mut high_value = MATE_VALUE;
        let mut high_depth = 0i32;
        let mut new_depth;
        let mut cut_move = 0i32;

        'search: {
            if exclusion {
                cnt = 0;
                singular = 0;
                flag = 1;
                score = beta - 1;
                hash_move = flags & 0xFFFF;
            } else {
                if (flags & FLAG_CALL_EVALUATION) != 0 {
                    self.evaluate();
                }
                if self.check(me) {
                    return self.search_evasion(
                        me,
                        false,
                        beta,
                        depth,
                        flags & !(FLAG_HALT_CHECK | FLAG_CALL_EVALUATION),
                    );
                }
                let mut value = self.score() - 90 - (depth << 3) - (i32::max(depth - 5, 0) << 5);
                if value >= beta
                    && (self.pawn(opp(me)) & self.line(me, 1) & Self::shift(me, !self.piece_all()))
                        == 0
                    && self.non_pawn_king(me) != 0
                    && (flags & (FLAG_RETURN_BEST_MOVE | FLAG_DISABLE_NULL)) == 0
                    && depth <= 13
                {
                    return value;
                }
                value = self.score() + 50;
                if value < beta && depth <= 3 {
                    let q = self.q_search(
                        me,
                        false,
                        beta - 1,
                        beta,
                        1,
                        FLAG_HASH_CHECK | (flags & 0xFFFF),
                    );
                    return value.max(q);
                }
                high_depth = 0;
                high_value = MATE_VALUE;
                hash_value = -MATE_VALUE;
                hash_depth = -1;
                hash_move = flags & 0xFFFF;
                self.pos.data[self.pos.cur].best = hash_move;
                if let Some(idx) = self.probe_hash() {
                    let e = self.hash.load(idx);
                    if (e.high_depth as i32) > high_depth {
                        high_depth = e.high_depth as i32;
                        high_value = e.high as i32;
                    }
                    if (e.high as i32) < beta && (e.high_depth as i32) >= depth {
                        return e.high as i32;
                    }
                    if e.mv != 0 && (e.low_depth as i32) > hash_depth {
                        hash_move = e.mv as i32;
                        self.pos.data[self.pos.cur].best = hash_move;
                        hash_depth = e.low_depth as i32;
                        if e.low_depth != 0 {
                            hash_value = e.low as i32;
                        }
                    }
                    if (e.low as i32) >= beta && (e.low_depth as i32) >= depth {
                        if e.mv != 0 {
                            let hm = e.mv as i32;
                            self.pos.data[self.pos.cur].best = hm;
                            if self.pos.board.square[move_to(hm)] == 0 && (hm & 0xE000) == 0 {
                                if self.pos.data[self.pos.cur].killer[1] as i32 != hm
                                    && (flags & FLAG_NO_KILLER_UPDATE) == 0
                                {
                                    self.pos.data[self.pos.cur].killer[2] =
                                        self.pos.data[self.pos.cur].killer[1];
                                    self.pos.data[self.pos.cur].killer[1] = hm as u16;
                                }
                                self.update_ref(hm);
                            }
                            return e.low as i32;
                        }
                        if (flags & FLAG_RETURN_BEST_MOVE) == 0 {
                            return e.low as i32;
                        }
                    }
                }
                // Tablebases intentionally omitted.
                if depth >= 20 {
                    if let Some(idx) = self.probe_pv_hash() {
                        let e = self.pv_hash.load(idx);
                        self.hash_low(e.mv as i32, e.value as i32, e.depth as i32);
                        self.hash_high(e.value as i32, e.depth as i32);
                        if (e.depth as i32) >= depth {
                            if e.mv != 0 {
                                self.pos.data[self.pos.cur].best = e.mv as i32;
                            }
                            let ply = self.pos.data[self.pos.cur].ply as i32;
                            if (flags & FLAG_RETURN_BEST_MOVE) == 0
                                && ((ply <= 50 && e.ply <= 50) || (ply >= 50 && e.ply >= 50))
                            {
                                return e.value as i32;
                            }
                        }
                        if e.mv != 0 && (e.depth as i32) > hash_depth {
                            hash_move = e.mv as i32;
                            self.pos.data[self.pos.cur].best = hash_move;
                            hash_depth = e.depth as i32;
                            hash_value = e.value as i32;
                        }
                    }
                }
                score = if depth < 10 {
                    height - MATE_VALUE
                } else {
                    beta - 1
                };
                if depth >= 12
                    && (hash_move == 0 || hash_value < beta || hash_depth < depth - 12)
                    && (high_value >= beta || high_depth < depth - 12)
                    && (flags & FLAG_DISABLE_NULL) == 0
                {
                    new_depth = depth - 8;
                    let value = self.search(
                        me,
                        false,
                        beta,
                        new_depth,
                        FLAG_HASH_CHECK
                            | FLAG_NO_KILLER_UPDATE
                            | FLAG_DISABLE_NULL
                            | FLAG_RETURN_BEST_MOVE
                            | hash_move,
                    );
                    if value >= beta {
                        if self.pos.data[self.pos.cur].best != 0 {
                            hash_move = self.pos.data[self.pos.cur].best;
                        }
                        hash_depth = new_depth;
                        hash_value = beta;
                    }
                }
                if depth >= 4
                    && self.score() + 3 >= beta
                    && (flags & (FLAG_DISABLE_NULL | FLAG_RETURN_BEST_MOVE)) == 0
                    && (high_value >= beta || high_depth < depth - 10)
                    && (depth < 12 || (hash_value >= beta && hash_depth >= depth - 12))
                    && beta > -EVAL_VALUE
                    && self.non_pawn_king(me) != 0
                {
                    new_depth = depth - 8;
                    self.do_null();
                    let value = -self.search(opp(me), false, 1 - beta, new_depth, FLAG_HASH_CHECK);
                    self.undo_null();
                    if value >= beta {
                        if depth < 12 {
                            self.hash_low(0, value, depth);
                        }
                        return value;
                    }
                }

                cnt = 0;
                flag = 0;
                singular = 0;
                if hash_move != 0 && self.is_legal(me, hash_move) {
                    if self.is_illegal(me, hash_move) {
                        // skip
                    } else {
                        cnt += 1;
                        let check = self.is_check(me, hash_move);
                        let mut ext = if check {
                            1 + i32::from(depth < 16)
                        } else {
                            self.extension(false, hash_move, depth)
                        };
                        new_depth = depth - i32::min(12, depth / 2);
                        if depth >= 16 && hash_value >= beta && hash_depth >= new_depth {
                            let margin_one = beta - EXCL_SINGLE;
                            let margin_two = beta - EXCL_DOUBLE;
                            let prev_ext = Self::ext(flags);
                            singular = self.singular_extension(
                                me, ext, prev_ext, margin_one, margin_two, new_depth, hash_move,
                            );
                            if singular != 0 {
                                ext = ext.max(
                                    singular + i32::from(prev_ext < 1)
                                        - i32::from(singular >= 2 && prev_ext >= 2),
                                );
                            }
                        }
                        let prev_mv = self.pos.data[self.pos.cur].mv as i32;
                        if depth < 16
                            && move_to(hash_move) == move_to(prev_mv)
                            && self.pos.board.square[move_to(hash_move)] != 0
                        {
                            ext = ext.max(2);
                        }
                        new_depth = depth - 2 + ext;
                        self.do_move(me, hash_move);
                        let mut f = FLAG_NEAT_SEARCH | Self::ext_flag(ext);
                        if hash_value >= beta && hash_depth >= depth - 12 {
                            f |= FLAG_DISABLE_NULL;
                        }
                        let value = -self.search(opp(me), false, 1 - beta, new_depth, f);
                        self.undo_move(me, hash_move);
                        if value > score {
                            score = value;
                            if value >= beta {
                                cut_move = hash_move;
                                break 'search;
                            }
                        }
                    }
                }
            }

            // skip_hash_move:
            self.pos.data[self.pos.cur].killer[0] = 0;
            self.pos.data[self.pos.cur].stage = STAGE_SEARCH;
            self.pos.data[self.pos.cur].gen_flags = 0;
            let pmv = self.pos.data[self.pos.cur].mv as i32;
            if pmv != 0 {
                let ri = self.ref_index(pmv);
                self.pos.data[self.pos.cur].refs[0] = self.reference[ri].refs[0];
                self.pos.data[self.pos.cur].refs[1] = self.reference[ri].refs[1];
            }
            let mut move_back = 0i32;
            if beta > 0 && self.pos.data[self.pos.cur].ply >= 2 && self.pos.cur > 0 {
                let pm = self.pos.data[self.pos.cur - 1].mv as i32;
                if (pm & 0xF000) == 0 {
                    move_back = (move_to(pm) as i32) << 6 | move_from(pm) as i32;
                    if self.pos.board.square[move_to(move_back)] != 0 {
                        move_back = 0;
                    }
                }
            }
            let moves_to_play = 3 + (depth * depth) / 6;
            let margin = self.score() + 70 + (depth << 3) + (i32::max(depth - 7, 0) << 5);
            if margin < beta && depth <= 19 {
                flag = 1;
                score = score.max(margin);
                self.pos.data[self.pos.cur].stage = STAGE_RAZORING;
                self.pos.data[self.pos.cur].mask = self.piece(opp(me));
                let value = self.score() + 200 + (depth << 5);
                if value < beta {
                    score = score.max(value);
                    self.pos.data[self.pos.cur].mask ^= self.pawn(opp(me));
                }
            }
            self.pos.data[self.pos.cur].current = 0;
            self.move_lists[self.pos.cur][0] = 0;
            if depth <= 5 {
                self.pos.data[self.pos.cur].gen_flags |= FLAG_NO_BC_SORT;
            }

            loop {
                let mv = self.get_move(me, false);
                if mv == 0 {
                    break;
                }
                if mv == hash_move {
                    continue;
                }
                if self.is_illegal(me, mv) {
                    continue;
                }
                cnt += 1;
                if mv == move_back {
                    score = score.max(0);
                    continue;
                }
                let st = self.pos.data[self.pos.cur];
                let check = if st.stage == R_CHECKS {
                    true
                } else {
                    self.is_check(me, mv)
                };
                let ext = if check && self.see(me, mv, 0) {
                    1 + i32::from(depth < 16)
                } else {
                    self.extension(false, mv, depth)
                };
                new_depth = depth - 2 + ext;
                if self.pos.board.square[move_to(mv)] == 0 && (mv & 0xE000) == 0 {
                    if mv != st.killer[1] as i32 && mv != st.killer[2] as i32 {
                        if !check && cnt > moves_to_play {
                            self.pos.data[self.pos.cur].gen_flags &= !FLAG_SORT;
                            continue;
                        }
                        if depth >= 6 {
                            let mut reduction = msb(cnt as u64) as i32;
                            if mv == st.refs[0] as i32 || mv == st.refs[1] as i32 {
                                reduction = i32::max(0, reduction - 1);
                            }
                            if reduction >= 2
                                && (self.queen(WHITE) | self.queen(BLACK)) == 0
                                && popcount(self.non_pawn_king_all()) <= 4
                            {
                                reduction += reduction / 2;
                            }
                            if new_depth - reduction > 3 && !self.see(me, mv, -50) {
                                reduction += 2;
                            }
                            if reduction != 0 && reduction < 2 && new_depth - reduction > 3 {
                                if cnt > 3 {
                                    reduction = 2;
                                } else {
                                    reduction = 0;
                                }
                            }
                            new_depth = i32::max(3, new_depth - reduction);
                        }
                    }
                    if !check {
                        let value = self.score() + self.delta_m(mv) + 10;
                        if value < beta && depth <= 3 {
                            score = score.max(value);
                            continue;
                        }
                        if cnt > 7 {
                            let value = margin + self.delta_m(mv) - 25 * msb(cnt as u64) as i32;
                            if value < beta && depth <= 19 {
                                score = score.max(value);
                                continue;
                            }
                        }
                    }
                    if depth <= 9 && self.non_pawn_king(me) != 0 && !self.see(me, mv, -50) {
                        continue;
                    }
                } else if self.pos.data[self.pos.cur].stage == R_CAP {
                    if !check && depth <= 9 && !self.see(me, mv, -50) {
                        continue;
                    }
                } else if self.pos.data[self.pos.cur].stage == S_BAD_CAP && !check && depth <= 5 {
                    continue;
                }
                self.do_move(me, mv);
                let value = -self.search(
                    opp(me),
                    false,
                    1 - beta,
                    new_depth,
                    FLAG_NEAT_SEARCH | Self::ext_flag(ext),
                );
                let value = if value >= beta && new_depth < depth - 2 + ext {
                    -self.search(
                        opp(me),
                        false,
                        1 - beta,
                        depth - 2 + ext,
                        FLAG_NEAT_SEARCH | FLAG_DISABLE_NULL | Self::ext_flag(ext),
                    )
                } else {
                    value
                };
                self.undo_move(me, mv);
                if value > score {
                    score = value;
                    if value >= beta {
                        cut_move = mv;
                        break 'search;
                    }
                }
            }
        }

        if cut_move != 0 {
            if exclusion {
                return score;
            }
            self.pos.data[self.pos.cur].best = cut_move;
            if depth >= 10 {
                score = i32::min(beta, score);
            }
            self.hash_low(cut_move, score, depth);
            if self.pos.board.square[move_to(cut_move)] == 0 && (cut_move & 0xE000) == 0 {
                if self.pos.data[self.pos.cur].killer[1] as i32 != cut_move
                    && (flags & FLAG_NO_KILLER_UPDATE) == 0
                {
                    self.pos.data[self.pos.cur].killer[2] = self.pos.data[self.pos.cur].killer[1];
                    self.pos.data[self.pos.cur].killer[1] = cut_move as u16;
                }
                self.history_good(cut_move, depth);
                if cut_move != hash_move && self.pos.data[self.pos.cur].stage == S_QUIET {
                    let start = self.pos.data[self.pos.cur].start;
                    let end = self.pos.data[self.pos.cur].current.saturating_sub(1);
                    for p in start..end {
                        let mv = self.move_lists[self.pos.cur][p];
                        self.history_bad(mv, depth);
                    }
                }
                self.update_ref(cut_move);
            }
            return score;
        }

        if cnt == 0 && flag == 0 {
            self.hash_high(0, 127);
            self.hash_low(0, 0, 127);
            return 0;
        }
        if !exclusion {
            self.hash_high(score, depth);
        }
        score
    }
}

impl Engine {
    #[allow(clippy::too_many_lines)]
    #[allow(unused_assignments)]
    pub fn search_evasion(
        &mut self,
        me: Color,
        exclusion: bool,
        beta: i32,
        depth: i32,
        flags: i32,
    ) -> i32 {
        let height = self.pos.cur as i32;
        if depth <= 1 {
            return self.q_evasion(me, false, beta - 1, beta, 1, flags);
        }
        let mut score = height - MATE_VALUE;
        if (flags & FLAG_HALT_CHECK) != 0 {
            if score >= beta {
                return beta;
            }
            if MATE_VALUE - height < beta {
                return beta - 1;
            }
            if let Some(v) = self.halt_check() {
                return v;
            }
        }
        let mut hash_value = -MATE_VALUE;
        let mut hash_depth = -1i32;
        let mut hash_move = flags & 0xFFFF;
        let mut cnt = 0i32;
        let mut pext = 0i32;
        let mut cut_move = 0i32;

        if exclusion {
            cnt = 0;
            pext = 0;
            score = beta - 1;
            self.gen_evasions(me);
            if self.move_lists[self.pos.cur][0] == 0 {
                return score;
            }
        } else {
            self.pos.data[self.pos.cur].best = hash_move;
            if let Some(idx) = self.probe_hash() {
                let e = self.hash.load(idx);
                if (e.high as i32) < beta && (e.high_depth as i32) >= depth {
                    return e.high as i32;
                }
                if e.mv != 0 && (e.low_depth as i32) > hash_depth {
                    hash_move = e.mv as i32;
                    self.pos.data[self.pos.cur].best = hash_move;
                    hash_depth = e.low_depth as i32;
                }
                if (e.low as i32) >= beta && (e.low_depth as i32) >= depth {
                    if e.mv != 0 {
                        let hm = e.mv as i32;
                        self.pos.data[self.pos.cur].best = hm;
                        if self.pos.board.square[move_to(hm)] == 0 && (hm & 0xE000) == 0 {
                            self.update_check_ref(hm);
                        }
                    }
                    return e.low as i32;
                }
                if (e.low_depth as i32) >= depth - 8 && (e.low as i32) > hash_value {
                    hash_value = e.low as i32;
                }
            }
            if depth >= 20 {
                if let Some(idx) = self.probe_pv_hash() {
                    let e = self.pv_hash.load(idx);
                    self.hash_low(e.mv as i32, e.value as i32, e.depth as i32);
                    self.hash_high(e.value as i32, e.depth as i32);
                    if (e.depth as i32) >= depth {
                        if e.mv != 0 {
                            self.pos.data[self.pos.cur].best = e.mv as i32;
                        }
                        return e.value as i32;
                    }
                    if e.mv != 0 && (e.depth as i32) > hash_depth {
                        hash_move = e.mv as i32;
                        self.pos.data[self.pos.cur].best = hash_move;
                        hash_depth = e.depth as i32;
                        hash_value = e.value as i32;
                    }
                }
            }
            // Tablebases intentionally omitted.
            if hash_depth >= depth && hash_value > -EVAL_VALUE {
                score = i32::min(beta - 1, i32::max(score, hash_value));
            }
            if (flags & FLAG_CALL_EVALUATION) != 0 {
                self.evaluate();
            }
            self.pos.data[self.pos.cur].mask = !0u64;
            if self.score() - 10 < beta && depth <= 3 {
                self.pos.data[self.pos.cur].mask = self.piece(opp(me));
                score = self.score() - 10;
                self.capture_margin(me, beta, &mut score);
            }
            cnt = 0;
            pext = 0;
            self.gen_evasions(me);
            if self.move_lists[self.pos.cur][0] == 0 {
                return score;
            }
            if self.move_lists[self.pos.cur][1] == 0 {
                pext = 2;
            }
            if hash_move != 0 && self.is_legal(me, hash_move) {
                if self.is_illegal(me, hash_move) {
                    // skip
                } else {
                    cnt += 1;
                    let check = self.is_check(me, hash_move);
                    let mut ext = if check {
                        i32::max(pext, 1 + i32::from(depth < 16))
                    } else {
                        i32::max(pext, self.extension(false, hash_move, depth))
                    };
                    let excl_depth = depth - i32::min(12, depth / 2);
                    if depth >= 16 && hash_value >= beta && hash_depth >= excl_depth {
                        let margin_one = beta - EXCL_SINGLE;
                        let margin_two = beta - EXCL_DOUBLE;
                        let prev_ext = Self::ext(flags);
                        let singular = self.singular_extension(
                            me, ext, prev_ext, margin_one, margin_two, excl_depth, hash_move,
                        );
                        if singular != 0 {
                            ext = i32::max(
                                ext,
                                singular + i32::from(prev_ext < 1)
                                    - i32::from(singular >= 2 && prev_ext >= 2),
                            );
                        }
                    }
                    let new_depth = depth - 2 + ext;
                    self.do_move(me, hash_move);
                    self.evaluate();
                    if (self.pos.data[self.pos.cur].att[opp(me)] & self.king(me)) != 0 {
                        self.undo_move(me, hash_move);
                        cnt -= 1;
                        // skip hash move
                    } else {
                        let mut f = FLAG_HALT_CHECK | FLAG_HASH_CHECK | Self::ext_flag(ext);
                        if hash_value >= beta && hash_depth >= depth - 12 {
                            f |= FLAG_DISABLE_NULL;
                        }
                        let value = -self.search(opp(me), false, 1 - beta, new_depth, f);
                        self.undo_move(me, hash_move);
                        if value > score {
                            score = value;
                            if value >= beta {
                                cut_move = hash_move;
                            }
                        }
                    }
                }
            }
        }

        'gen: {
            if cut_move != 0 {
                break 'gen;
            }
            let moves_to_play = 3 + (depth * depth) / 6;
            let pmv = self.pos.data[self.pos.cur].mv as i32;
            if pmv != 0 {
                let ri = self.ref_index(pmv);
                self.pos.data[self.pos.cur].refs[0] = self.reference[ri].check_refs[0];
                self.pos.data[self.pos.cur].refs[1] = self.reference[ri].check_refs[1];
            }
            self.mark_evasions();
            self.pos.data[self.pos.cur].current = 0;
            loop {
                let mv = self.pick_move();
                if mv == 0 {
                    break;
                }
                if mv == hash_move {
                    continue;
                }
                if self.is_illegal(me, mv) {
                    continue;
                }
                cnt += 1;
                if self.is_repetition(beta, mv) {
                    score = score.max(0);
                    continue;
                }
                let check = self.is_check(me, mv);
                let ext = if check {
                    i32::max(pext, 1 + i32::from(depth < 16))
                } else {
                    i32::max(pext, self.extension(false, mv, depth))
                };
                let mut new_depth = depth - 2 + ext;
                if self.pos.board.square[move_to(mv)] == 0 && (mv & 0xE000) == 0 {
                    if !check {
                        if cnt > moves_to_play {
                            continue;
                        }
                        let value = self.score() + self.delta_m(mv) + 10;
                        if value < beta && depth <= 3 {
                            score = score.max(value);
                            continue;
                        }
                    }
                    if depth >= 6 && cnt > 3 {
                        let mut reduction = msb(cnt as u64) as i32;
                        if reduction >= 2
                            && (self.queen(WHITE) | self.queen(BLACK)) == 0
                            && popcount(self.non_pawn_king_all()) <= 4
                        {
                            reduction += reduction / 2;
                        }
                        new_depth = i32::max(3, new_depth - reduction);
                    }
                }
                self.do_move(me, mv);
                let value = -self.search(
                    opp(me),
                    false,
                    1 - beta,
                    new_depth,
                    FLAG_NEAT_SEARCH | Self::ext_flag(ext),
                );
                let value = if value >= beta && new_depth < depth - 2 + ext {
                    -self.search(
                        opp(me),
                        false,
                        1 - beta,
                        depth - 2 + ext,
                        FLAG_NEAT_SEARCH | FLAG_DISABLE_NULL | Self::ext_flag(ext),
                    )
                } else {
                    value
                };
                self.undo_move(me, mv);
                if value > score {
                    score = value;
                    if value >= beta {
                        cut_move = mv;
                        break 'gen;
                    }
                }
            }
        }

        if cut_move != 0 {
            if exclusion {
                return score;
            }
            self.pos.data[self.pos.cur].best = cut_move;
            self.hash_low(cut_move, score, depth);
            if self.pos.board.square[move_to(cut_move)] == 0 && (cut_move & 0xE000) == 0 {
                self.update_check_ref(cut_move);
            }
            return score;
        }
        if !exclusion {
            self.hash_high(score, depth);
        }
        score
    }
}

// ---------------------------------------------------------------------------
// Principal variation search
// ---------------------------------------------------------------------------

impl Engine {}

impl Engine {
    #[allow(clippy::too_many_lines)]
    #[allow(unused_assignments)]
    pub fn pv_search(
        &mut self,
        me: Color,
        root: bool,
        mut alpha: i32,
        beta: i32,
        mut depth: i32,
        mut flags: i32,
    ) -> i32 {
        let height = self.pos.cur as i32;
        let mut pext = 0i32;
        let mut ext;
        let mut hash_value = -MATE_VALUE;
        let mut singular = 0i32;
        let mut new_depth;
        let mut hash_move;
        let mut hash_depth;
        let old_alpha = alpha;
        let mut ex_depth = 0i32;
        let mut ex_value = 0i32;
        let start_knodes = (self.pos.nodes >> 10) as i32;
        let mut cnt = 0i32;
        let mut cut_move = 0i32;

        if root {
            depth = i32::max(depth, 2);
            flags |= Self::ext_flag(1);
            if self.root_list[0] == 0 {
                return 0;
            }
            self.reporter.info_depth(depth / 2);
            let mut p = 0usize;
            while self.root_list[p] != 0 {
                p += 1;
            }
            sort_moves(0, p, &mut self.root_list);
            for i in 0..p {
                self.root_list[i] &= 0xFFFF;
            }
            self.root_list[0] = set_score(self.root_list[0], 2);
        } else {
            if depth <= 1 {
                return self.q_search(me, true, alpha, beta, 1, FLAG_NEAT_SEARCH);
            }
            if height - MATE_VALUE >= beta {
                return beta;
            }
            if MATE_VALUE - height <= alpha {
                return alpha;
            }
            if let Some(v) = self.halt_check() {
                return v;
            }
        }

        // check_hash:
        hash_depth = -1;
        hash_move = 0;
        self.pos.data[self.pos.cur].best = 0;
        if let Some(idx) = self.probe_pv_hash() {
            let e = self.pv_hash.load(idx);
            self.hash_low(e.mv as i32, e.value as i32, e.depth as i32);
            self.hash_high(e.value as i32, e.depth as i32);
            if (e.depth as i32) >= depth && self.pv_hashing != 0 {
                if e.mv != 0 {
                    self.pos.data[self.pos.cur].best = e.mv as i32;
                }
                let ply = self.pos.data[self.pos.cur].ply as i32;
                if (ply <= 50 && e.ply <= 50) || (ply >= 50 && e.ply >= 50) {
                    if e.value == 0 || !self.draw_in_pv(me) {
                        return e.value as i32;
                    }
                }
            }
            if e.mv != 0 && (e.depth as i32) > hash_depth {
                hash_move = e.mv as i32;
                self.pos.data[self.pos.cur].best = hash_move;
                hash_depth = e.depth as i32;
                hash_value = e.value as i32;
            }
        }
        if let Some(idx) = self.probe_hash() {
            let e = self.hash.load(idx);
            if e.mv != 0 && (e.low_depth as i32) > hash_depth {
                hash_move = e.mv as i32;
                self.pos.data[self.pos.cur].best = hash_move;
                hash_depth = e.low_depth as i32;
                if e.low_depth != 0 {
                    hash_value = e.low as i32;
                }
            }
        }
        // Tablebases intentionally omitted.
        if root {
            hash_move = self.root_list[0];
            hash_value = self.previous;
            hash_depth = i32::max(0, depth - 2);
        }

        self.evaluate();

        if !root && depth >= 6 && (hash_move == 0 || hash_value <= alpha || hash_depth < depth - 8)
        {
            let mut iid = 0;
            let mut value;
            let mut skip_iid = false;
            if hash_move == 0 {
                new_depth = depth - 2;
            } else {
                new_depth = depth - 4;
            }
            value = self.pv_search(me, false, alpha, beta, new_depth, hash_move);
            if value > alpha {
                if self.pos.data[self.pos.cur].best != 0 {
                    hash_move = self.pos.data[self.pos.cur].best;
                }
                hash_depth = new_depth;
                hash_value = value;
                skip_iid = true;
            } else {
                new_depth = depth - 8;
                loop {
                    let margin = alpha - (8 << iid);
                    if hash_move != 0
                        && hash_depth >= i32::min(new_depth, depth - 8)
                        && hash_value >= margin
                    {
                        skip_iid = true;
                        break;
                    }
                    value = self.search(
                        me,
                        false,
                        margin,
                        new_depth,
                        FLAG_HASH_CHECK
                            | FLAG_NO_KILLER_UPDATE
                            | FLAG_DISABLE_NULL
                            | FLAG_RETURN_BEST_MOVE
                            | hash_move,
                    );
                    if value >= margin {
                        if self.pos.data[self.pos.cur].best != 0 {
                            hash_move = self.pos.data[self.pos.cur].best;
                        }
                        hash_depth = new_depth;
                        hash_value = value;
                        skip_iid = true;
                        break;
                    }
                    iid += 1;
                    if iid >= 5 {
                        break;
                    }
                }
            }
            let _ = skip_iid;
        }
        // skip_iid:

        if !root && self.check(me) {
            alpha = i32::max(height - MATE_VALUE, alpha);
            self.pos.data[self.pos.cur].mask = !0u64;
            self.gen_evasions(me);
            if self.move_lists[self.pos.cur][0] == 0 {
                return height - MATE_VALUE;
            }
            if self.move_lists[self.pos.cur][1] == 0 {
                pext = 2;
            }
        }

        'pv: {
            if hash_move != 0 && self.is_legal(me, hash_move) {
                cnt += 1;
                if root {
                    for s in self.pos.data.iter_mut().skip(1).take(127) {
                        *s = State::default();
                    }
                    self.send_curr_move(hash_move, cnt);
                }
                let check = self.is_check(me, hash_move);
                ext = if check {
                    2
                } else {
                    i32::max(pext, self.extension(true, hash_move, depth))
                };
                new_depth = depth - i32::min(12, depth / 2);
                if depth >= 12 && hash_value > alpha && hash_depth >= new_depth {
                    let margin_one = hash_value - EXCL_SINGLE_PV;
                    let margin_two = hash_value - EXCL_DOUBLE_PV;
                    let prev_ext = Self::ext(flags);
                    singular = self.singular_extension(
                        me,
                        if root { 0 } else { ext },
                        if root { 0 } else { prev_ext },
                        margin_one,
                        margin_two,
                        new_depth,
                        hash_move,
                    );
                    if singular != 0 {
                        ext = i32::max(
                            ext,
                            singular + i32::from(prev_ext < 1)
                                - i32::from(singular >= 2 && prev_ext >= 2),
                        );
                        if root {
                            self.si.singular = singular;
                        }
                        ex_depth = new_depth;
                        ex_value = (if singular >= 2 {
                            margin_two
                        } else {
                            margin_one
                        }) - 1;
                    }
                }
                new_depth = depth - 2 + ext;
                self.do_move(me, hash_move);
                let value = -self.pv_search(
                    opp(me),
                    false,
                    -beta,
                    -alpha,
                    new_depth,
                    Self::ext_flag(ext),
                );
                self.undo_move(me, hash_move);
                if value > alpha {
                    if root {
                        self.si.fail_low = 0;
                        self.stats.best_move = hash_move;
                        self.stats.best_score = value;
                        self.hash_low(hash_move, value, depth);
                        if depth >= 14 {
                            self.send_pv(depth / 2, old_alpha, beta, value);
                        }
                    }
                    alpha = value;
                    self.pos.data[self.pos.cur].best = hash_move;
                    if value >= beta {
                        cut_move = hash_move;
                        break 'pv;
                    }
                } else if root {
                    self.si.fail_low = 1;
                    self.si.fail_high = 0;
                    self.si.singular = 0;
                    if depth >= 14 {
                        self.send_pv(depth / 2, old_alpha, beta, value);
                    }
                }
            }

            // skip_hash_move:
            self.pos.data[self.pos.cur].gen_flags = 0;
            if !self.check(me) {
                self.pos.data[self.pos.cur].stage = STAGE_SEARCH;
                let pmv = self.pos.data[self.pos.cur].mv as i32;
                if pmv != 0 {
                    let ri = self.ref_index(pmv);
                    self.pos.data[self.pos.cur].refs[0] = self.reference[ri].refs[0];
                    self.pos.data[self.pos.cur].refs[1] = self.reference[ri].refs[1];
                }
            } else {
                self.pos.data[self.pos.cur].stage = STAGE_EVASION;
                let pmv = self.pos.data[self.pos.cur].mv as i32;
                if pmv != 0 {
                    let ri = self.ref_index(pmv);
                    self.pos.data[self.pos.cur].refs[0] = self.reference[ri].check_refs[0];
                    self.pos.data[self.pos.cur].refs[1] = self.reference[ri].check_refs[1];
                }
            }
            self.pos.data[self.pos.cur].killer[0] = 0;
            self.move_lists[self.pos.cur][0] = 0;
            if root {
                self.pos.data[self.pos.cur].current = 1;
            } else {
                self.pos.data[self.pos.cur].current = 0;
            }

            loop {
                let mv = self.get_move(me, root);
                if mv == 0 {
                    break;
                }
                if mv == hash_move {
                    continue;
                }
                if self.is_illegal(me, mv) {
                    continue;
                }
                cnt += 1;
                if root {
                    for s in self.pos.data.iter_mut().skip(1).take(127) {
                        *s = State::default();
                    }
                    self.send_curr_move(mv, cnt);
                }
                if self.is_repetition(alpha + 1, mv) {
                    continue;
                }
                let check = self.is_check(me, mv);
                ext = if check {
                    2
                } else {
                    i32::max(pext, self.extension(true, mv, depth))
                };
                new_depth = depth - 2 + ext;
                let st = self.pos.data[self.pos.cur];
                if depth >= 6
                    && (mv & 0xE000) == 0
                    && self.pos.board.square[move_to(mv)] == 0
                    && (root
                        || (mv != st.killer[1] as i32 && mv != st.killer[2] as i32)
                        || self.check(me))
                    && cnt > 3
                {
                    let mut reduction = msb(cnt as u64) as i32 - 1;
                    if mv == st.refs[0] as i32 || mv == st.refs[1] as i32 {
                        reduction = i32::max(0, reduction - 1);
                    }
                    if reduction >= 2
                        && (self.queen(WHITE) | self.queen(BLACK)) == 0
                        && popcount(self.non_pawn_king_all()) <= 4
                    {
                        reduction += reduction / 2;
                    }
                    new_depth = i32::max(3, new_depth - reduction);
                }
                self.do_move(me, mv);
                let mut value = if new_depth <= 1 {
                    -self.pv_search(
                        opp(me),
                        false,
                        -beta,
                        -alpha,
                        new_depth,
                        Self::ext_flag(ext),
                    )
                } else {
                    -self.search(
                        opp(me),
                        false,
                        -alpha,
                        new_depth,
                        FLAG_NEAT_SEARCH | Self::ext_flag(ext),
                    )
                };
                if value > alpha && new_depth > 1 {
                    let old_best = self.stats.best_move;
                    if root {
                        self.root_list[(cnt - 1) as usize] =
                            set_score(self.root_list[(cnt - 1) as usize], 1);
                        self.si.early = 0;
                        self.stats.best_move = mv;
                    }
                    new_depth = depth - 2 + ext;
                    value = -self.pv_search(
                        opp(me),
                        false,
                        -beta,
                        -alpha,
                        new_depth,
                        Self::ext_flag(ext),
                    );
                    if root && value <= alpha {
                        self.stats.best_move = old_best;
                    }
                }
                self.undo_move(me, mv);
                if value > alpha {
                    if root {
                        self.root_list[(cnt - 1) as usize] =
                            set_score(self.root_list[(cnt - 1) as usize], cnt + 3);
                        self.si.change = 1;
                        self.si.fail_low = 0;
                        self.stats.best_move = mv;
                        self.stats.best_score = value;
                        if depth >= 14 {
                            self.send_pv(depth / 2, old_alpha, beta, value);
                        }
                    }
                    alpha = value;
                    self.pos.data[self.pos.cur].best = mv;
                    if value >= beta {
                        cut_move = mv;
                        break 'pv;
                    }
                }
            }
        }

        if cut_move != 0 {
            self.hash_low(cut_move, alpha, depth);
            return alpha;
        }
        if cnt == 0 && !self.check(me) {
            self.hash_high(0, 127);
            self.hash_low(0, 0, 127);
            self.hash_exact(0, 0, 127, 0, 0, 0);
            return 0;
        }
        if !root || self.search_moves == 0 {
            self.hash_high(alpha, depth);
        }
        if alpha > old_alpha {
            let best = self.pos.data[self.pos.cur].best;
            self.hash_low(best, alpha, depth);
            if best != hash_move {
                ex_depth = 0;
            }
            if !root || self.search_moves == 0 {
                self.hash_exact(
                    best,
                    alpha,
                    depth,
                    ex_value,
                    ex_depth,
                    (self.pos.nodes >> 10) as i32 - start_knodes,
                );
            }
        }
        alpha
    }

    /// `multipv(depth)`
    pub fn multipv(&mut self, me: Color, depth: i32) -> i32 {
        let mut low = MATE_VALUE;
        let mut new_depth;
        let mut ext;
        let pvn = self.settings.multi_pv;
        let mut cnt = 0usize;
        while cnt < pvn {
            let m = self.multi_pv[cnt];
            let mv = m & 0xFFFF;
            if mv == 0 {
                break;
            }
            self.multi_pv[cnt] = mv;
            ext = self.extension(true, mv, depth);
            new_depth = depth - 2 + ext;
            self.do_move(me, mv);
            let value = -self.pv_search(
                opp(me),
                false,
                -MATE_VALUE,
                MATE_VALUE,
                new_depth,
                Self::ext_flag(ext),
            );
            self.multi_pv[cnt] |= value << 16;
            if value < low {
                low = value;
            }
            self.undo_move(me, mv);
            let mut i = cnt as i32 - 1;
            while i >= 0 {
                if (self.multi_pv[i as usize] >> 16) < value {
                    self.multi_pv[(i + 1) as usize] = self.multi_pv[i as usize];
                    self.multi_pv[i as usize] = mv | (value << 16);
                }
                i -= 1;
            }
            self.stats.best_move = self.multi_pv[0] & 0xFFFF;
            self.pos.data[self.pos.cur].score = (self.multi_pv[0] >> 16) as i16;
            self.send_multipv(depth / 2, cnt as i32);
            cnt += 1;
        }
        while cnt < 256 {
            let m = self.multi_pv[cnt];
            let mv = m & 0xFFFF;
            if mv == 0 {
                break;
            }
            self.multi_pv[cnt] = mv;
            ext = self.extension(true, mv, depth);
            new_depth = depth - 2 + ext;
            self.do_move(me, mv);
            let mut value = -self.search(
                opp(me),
                false,
                -low,
                new_depth,
                FLAG_NEAT_SEARCH | Self::ext_flag(ext),
            );
            if value > low {
                value = -self.pv_search(
                    opp(me),
                    false,
                    -MATE_VALUE,
                    -low,
                    new_depth,
                    Self::ext_flag(ext),
                );
            }
            self.multi_pv[cnt] |= value << 16;
            self.undo_move(me, mv);
            if value > low {
                let mut i = cnt as i32;
                while i >= pvn as i32 {
                    self.multi_pv[i as usize] = self.multi_pv[(i - 1) as usize];
                    i -= 1;
                }
                self.multi_pv[pvn - 1] = mv | (value << 16);
                let mut i = pvn as i32 - 2;
                while i >= 0 {
                    if (self.multi_pv[i as usize] >> 16) < value {
                        self.multi_pv[(i + 1) as usize] = self.multi_pv[i as usize];
                        self.multi_pv[i as usize] = mv | (value << 16);
                    }
                    i -= 1;
                }
                self.stats.best_move = self.multi_pv[0] & 0xFFFF;
                self.pos.data[self.pos.cur].score = (self.multi_pv[0] >> 16) as i16;
                low = self.multi_pv[pvn - 1] >> 16;
                self.send_multipv(depth / 2, cnt as i32);
            }
            cnt += 1;
        }
        self.pos.data[self.pos.cur].score as i32
    }
}

// ---------------------------------------------------------------------------
// Iterative deepening root
// ---------------------------------------------------------------------------

pub const SCHEDULE: [u32; 20] = [
    0x00FF_FFFF,
    0x0055_5555,
    0x00AA_AAAA,
    0x0033_3333,
    0x0066_6666,
    0x00CC_CCCC,
    0x001C_71C7,
    0x0038_E38E,
    0x0071_C71C,
    0x00E3_8E38,
    0x00C7_1C71,
    0x008E_38E3,
    0x000F_0F0F,
    0x001E_1E1E,
    0x003C_3C3C,
    0x0078_7878,
    0x00F0_F0F0,
    0x00E1_E1E1,
    0x00C3_C3C3,
    0x0087_8787,
];

impl Engine {
    pub fn root(&mut self) {
        let me = self.pos.data[self.pos.cur].turn as Color;
        self.root_color(me);
    }

    #[allow(clippy::too_many_lines)]
    #[allow(unused_assignments)]
    pub fn root_color(&mut self, me: Color) {
        let mut start_depth = 2i32;
        let mut hash_depth = 0i32;
        let mut hash_value = 0i32;
        let store_time = 0i32;
        let mut ex_depth = 0i32;
        let mut ex_value = 0i32;
        let mut prev_time = 0i64;
        let mut knodes = 0i32;

        self.evaluate();
        self.gen_root_moves(me);
        if self.thread_id != 0 {
            let n = self
                .root_list
                .iter()
                .position(|&m| m == 0)
                .unwrap_or(self.root_list.len());
            if n > 1 {
                let shift = self.thread_id as usize % n;
                self.root_list[..n].rotate_left(shift);
            }
        }
        if self.settings.multi_pv > 1 {
            self.multi_pv = [0; 256];
            let mut i = 0;
            while self.root_list[i] != 0 {
                self.multi_pv[i] = self.root_list[i];
                i += 1;
            }
        }
        self.stats.best_move = self.root_list[0];
        if self.root_list[0] == 0 {
            self.stats.stop = true;
            return;
        }
        if self.root_list[1] == 0 {
            let value = self.pv_search(me, true, -MATE_VALUE, MATE_VALUE, 4, FLAG_NEAT_SEARCH);
            self.last_depth = 128;
            self.send_pv(6, -MATE_VALUE, MATE_VALUE, value);
            self.stats.stop = true;
            return;
        }

        self.si = SearchInfo::default();
        self.base_si = SearchInfo::default();
        self.previous = -MATE_VALUE;
        if let Some(idx) = self.probe_pv_hash() {
            let e = self.pv_hash.load(idx);
            if e.mv != 0
                && self.is_legal(me, e.mv as i32)
                && e.mv as i32 == self.stats.best_move
                && (e.depth as i32) > hash_depth
            {
                hash_depth = e.depth as i32;
                hash_value = e.value as i32;
                ex_depth = e.ex_depth as i32;
                ex_value = e.exclusion as i32;
                knodes = e.knodes;
            }
        }
        let mut depth = start_depth;
        if hash_depth != 0 && self.settings.multi_pv == 1 {
            self.previous = hash_value;
            self.stats.best_score = hash_value;
            depth = hash_depth;
            if self.pv_hashing != 0 {
                self.send_pv(depth / 2, -MATE_VALUE, MATE_VALUE, self.stats.best_score);
                start_depth = (depth + 2) & !1;
                depth = start_depth;
            }
            if (depth >= self.last_depth - 8 || store_time != 0)
                && self.last_value >= self.last_exact_value
                && hash_value >= self.last_exact_value
                && self.last_time != 0
                && self.last_speed != 0
            {
                let mut time = self.shared.soft_time_limit;
                if ex_depth >= depth - i32::min(12, depth / 2)
                    && ex_value <= hash_value - EXCL_SINGLE_PV
                {
                    self.base_si.early = 1;
                    self.base_si.singular = 1;
                    if ex_value <= hash_value - EXCL_DOUBLE_PV {
                        time = (time * TIME_SING_TWO_MARGIN) / 100;
                        self.base_si.singular = 2;
                    } else {
                        time = (time * TIME_SING_ONE_MARGIN) / 100;
                    }
                }
                let _ = time;
                let mut time_est =
                    i64::min(self.last_time, (knodes as i64) << 10) / self.last_speed;
                // The C++ code uses Min(LastTime, (knodes << 10) / LastSpeed)
                // when LastSpeed is non-zero.
                time_est = i64::max(time_est, store_time as i64);
                prev_time = time_est;
                self.last_time = time_est;
            } else {
                self.last_time = 0;
            }
        } else {
            self.last_time = 0;
        }

        // set_jump:
        self.save_board = self.pos.board;
        self.save_state = self.pos.data[0];
        self.save_sp = self.pos.sp;

        let mut value = 0i32;
        while depth < self.shared.depth_limit as i32 && depth < 256 {
            self.stats.depth = depth;
            let sched = SCHEDULE[(self.stats.id as usize) % SCHEDULE.len()];
            let idx = ((self.shared.root_depth.wrapping_add((depth / 2) as u32)) % 24) as u32;
            if ((sched >> idx) & 1) == 0 {
                depth += 2;
                continue;
            }
            for s in self.pos.data.iter_mut().skip(1).take(127) {
                *s = State::default();
            }
            self.si.early = 1;
            self.si.change = 0;
            self.si.fail_high = 0;
            self.si.fail_low = 0;
            self.si.singular = 0;

            if self.settings.multi_pv > 1 {
                value = self.multipv(me, depth);
                self.last_value = value;
                self.last_exact_value = value;
            } else if depth / 2 < 7 || self.aspiration == 0 {
                value = self.pv_search(me, true, -MATE_VALUE, MATE_VALUE, depth, FLAG_NEAT_SEARCH);
                self.last_value = value;
                self.last_exact_value = value;
            } else {
                let mut delta = 8i32;
                let mut alpha = self.previous - delta;
                let mut beta = self.previous + delta;
                loop {
                    if delta >= 16 * 32 {
                        value = self.pv_search(
                            me,
                            true,
                            -MATE_VALUE,
                            MATE_VALUE,
                            depth,
                            FLAG_NEAT_SEARCH,
                        );
                        self.last_value = value;
                        self.last_exact_value = value;
                        break;
                    }
                    value = self.pv_search(me, true, alpha, beta, depth, FLAG_NEAT_SEARCH);
                    if value <= alpha {
                        self.si.fail_high = 0;
                        self.si.fail_low = 1;
                        alpha -= delta;
                        delta *= 2;
                        self.last_value = value;
                        self.base_si = self.si;
                        continue;
                    } else if value >= beta {
                        self.si.fail_high = 1;
                        self.si.fail_low = 0;
                        self.si.early = 1;
                        self.si.change = 0;
                        self.si.singular = self.si.singular.max(1);
                        beta += delta;
                        delta *= 2;
                        self.last_depth = depth;
                        let now = now_ms();
                        self.last_time =
                            i64::max(prev_time, now.saturating_sub(self.shared.start_time) as i64);
                        self.last_speed =
                            (self.pos.nodes / i64::max(self.last_time, 1) as u64) as i64;
                        if depth + 2 < self.shared.depth_limit as i32 {
                            depth += 2;
                            self.stats.depth = depth;
                        }
                        self.inst_cnt = 0;
                        if self.stats.id == 0 && self.time_to_stop(&self.si, self.last_time, false)
                        {
                            self.stats.stop = true;
                            break;
                        }
                        for s in self.pos.data.iter_mut().skip(1).take(127) {
                            *s = State::default();
                        }
                        self.last_value = value;
                        self.base_si = self.si;
                        continue;
                    } else {
                        self.last_value = value;
                        self.last_exact_value = value;
                    }
                    break;
                }
                if self.stats.stop {
                    break;
                }
            }
            // finish:
            if value < self.previous - 50 {
                self.si.bad = 1;
            } else {
                self.si.bad = 0;
            }
            self.base_si = self.si;
            self.last_depth = depth;
            let now = now_ms();
            self.last_time = i64::max(prev_time, now.saturating_sub(self.shared.start_time) as i64);
            self.last_speed = (self.pos.nodes / i64::max(self.last_time, 1) as u64) as i64;
            self.previous = value;
            self.inst_cnt = 0;
            if self.stats.id == 0 && self.time_to_stop(&self.si, self.last_time, false) {
                self.stats.stop = true;
                break;
            }
            depth += 2;
        }

        // stop:
        self.pos.cur = 0;
        self.pos.board = self.save_board;
        self.pos.data[0] = self.save_state;
        self.pos.sp = self.save_sp;
        if !self.stats.stop {
            self.stats.stop = true;
        }
        let _ = (start_depth, prev_time, hash_depth);
    }

    pub fn result(&self) -> SearchResult {
        let mut pv = Vec::new();
        for &m in self.stats.pv.iter() {
            if m == 0 {
                break;
            }
            pv.push(m);
        }
        SearchResult {
            best_move: if self.stats.best_move != 0 {
                self.stats.best_move
            } else {
                self.root_list[0]
            },
            ponder_move: if self.stats.pv[0] != 0 {
                self.stats.pv[1]
            } else {
                0
            },
            score: self.stats.best_score,
            nodes: self.pos.nodes,
            depth: self.last_depth / 2,
            sel_depth: self.stats.sel_depth / 2,
            pv,
        }
    }

    fn apply_limits(&mut self, limits: &SearchLimits) {
        self.shared.depth_limit = 2 * limits.depth + 2;
        self.shared.node_limit = limits.nodes;
        self.shared.ponder = limits.ponder;
        let turn = self.pos.data[self.pos.cur].turn as Color;
        let time = if turn == WHITE {
            limits.wtime
        } else {
            limits.btime
        };
        let inc = if turn == WHITE {
            limits.winc
        } else {
            limits.binc
        };
        let mut infinite = limits.infinite;
        if limits.movetime.is_none() && time == 0 && inc == 0 {
            infinite = true;
        }
        let movestogo = if limits.movestogo != 0 {
            limits.movestogo.saturating_sub(1).max(1)
        } else {
            0
        };
        let time_max = (time as i64 - i64::min(1000, (time / 2) as i64)).max(0) as u64;
        let mut nmoves = MOVES_TG as u64 - 1;
        let mut exp_moves = MOVES_TG - 1;
        if movestogo != 0 {
            nmoves = movestogo;
        } else {
            let ply = self.pos.data[self.pos.cur].ply as i64;
            if ply > 40 {
                nmoves += i64::min(ply - 40, (100 - ply) / 2) as u64;
            }
            exp_moves = nmoves as i64;
        }
        let min_moves = i64::min(exp_moves, nmoves as i64).max(1) as u64;
        let mut soft = u64::min(time_max, (time_max + min_moves * inc) / min_moves);
        let hard = u64::min(
            time_max,
            (time_max + u64::min(3, min_moves) * inc) / u64::min(3, min_moves),
        );
        soft = u64::min(time_max, (soft * TIME_RATIO) / 100);
        let (soft, hard) = if let Some(mt) = limits.movetime {
            (mt, mt)
        } else if infinite {
            (INFINITE_TIME, INFINITE_TIME)
        } else {
            (soft, hard)
        };
        self.shared.soft_time_limit = soft;
        self.shared.hard_time_limit = hard;
    }

    /// Run a search to completion; uses Lazy SMP when `Threads` > 1.
    pub fn go(&mut self, limits: &SearchLimits) -> SearchResult {
        let threads = self.settings.num_threads.clamp(1, 256);
        if threads <= 1 {
            self.go_single_at(limits, now_ms())
        } else {
            self.go_smp(limits, threads)
        }
    }

    fn go_single_at(&mut self, limits: &SearchLimits, start_time: u64) -> SearchResult {
        self.apply_limits(limits);
        self.pos.nodes = 0;
        self.pos.tb_hits = 0;
        self.stats.stop = false;
        self.stats.pv = [0; 64];
        self.stats.id = self.thread_id as i32;
        self.shared.start_time = start_time;
        self.save_root();
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.root();
        }));
        if result.is_err() {
            self.restore_shared_root();
            self.stats.stop = true;
        }
        self.result()
    }

    fn go_smp(&mut self, limits: &SearchLimits, threads: usize) -> SearchResult {
        let start_time = now_ms();
        // Workers use a smaller private pawn hash; the main engine keeps the
        // full-size one.  This keeps Threads=256 within reasonable memory.
        let worker_pawn_entries = 1usize << 14;
        let mut handles = Vec::with_capacity(threads - 1);
        for id in 1..threads {
            let mut worker = self.fork_for_thread(id as u32, worker_pawn_entries);
            let lim = limits.clone();
            let handle = std::thread::Builder::new()
                .stack_size(16 * 1024 * 1024)
                .spawn(move || {
                    let result = worker.go_single_at(&lim, start_time);
                    (worker, result)
                });
            if let Ok(handle) = handle {
                handles.push(handle);
            }
        }

        let mut main_result = self.go_single_at(limits, start_time);

        // Stop and collect workers.  The main thread's result is the one
        // reported to the UCI layer, but node counts are accumulated.
        self.control.request_stop();
        for handle in handles {
            if let Ok((_worker, result)) = handle.join() {
                main_result.nodes = main_result.nodes.saturating_add(result.nodes);
            }
        }
        self.control.reset();
        main_result
    }
}

#[cfg(test)]
mod extra_search_tests {
    use crate::position::Engine;
    use crate::search::{NullReporter, SearchLimits};
    use crate::types::*;

    #[test]
    fn see_recognises_free_capture() {
        let mut e = Engine::with_hash_mb(1);
        e.get_board("4k3/8/8/3q4/8/8/3R4/4K3 w - - 0 1");
        e.evaluate();
        let me = e.pos.cur().turn as Color;
        let cap = make_move(11, 35); // Rxd5
        assert!(e.see(me, cap, 0));
    }

    #[test]
    fn hash_store_and_probe_round_trip() {
        let mut e = Engine::with_hash_mb(1);
        e.get_board("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
        e.hash_low(0x1234, 42, 7);
        e.hash_high(50, 9);
        let idx = e.probe_hash().expect("hash entry");
        assert_eq!(e.hash.load(idx).key, e.pos.cur().key as u32);
    }

    #[test]
    fn finds_mate_in_one() {
        let mut e = Engine::with_hash_mb(1);
        e.reporter = Box::new(NullReporter);
        e.get_board("6k1/5ppp/8/8/8/8/5PPP/R5K1 w - - 0 1");
        let r = e.go(&SearchLimits {
            depth: 1,
            infinite: true,
            ..SearchLimits::default()
        });
        let (kind, value) = uci_score(r.score);
        assert_eq!(move_to_string(r.best_move), "a1a8");
        assert_eq!(kind, "mate");
        assert!(value >= 1, "mate score {kind} {value}");
    }

    #[test]
    fn qsearch_never_below_static_eval_for_side_to_move() {
        let mut e = Engine::with_hash_mb(1);
        e.get_board("r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 3");
        e.evaluate();
        let me = e.pos.cur().turn as Color;
        let eval = e.pos.cur().score as i32;
        let q = e.q_search(me, false, -MATE_VALUE, MATE_VALUE, 0, FLAG_NEAT_SEARCH);
        assert!(q >= eval - 1, "qsearch {q} < eval {eval}");
    }
}

#[cfg(test)]
mod legality_tests {
    use crate::position::Engine;
    use crate::types::*;

    #[test]
    fn pinned_pawn_capture_is_rejected_for_pv() {
        let mut e = Engine::with_hash_mb(1);
        // The white e2 pawn is pinned by the rook on e8.  Moving along the
        // e-file is legal, capturing on d3 is not.
        e.get_board("k3r3/8/8/8/8/3p4/4P3/4K3 w - - 0 1");
        e.evaluate();
        assert!(e.is_illegal(WHITE, make_move(12, 19))); // e2xd3
        assert!(!e.is_illegal(WHITE, make_move(12, 20))); // e2e3
    }
}
