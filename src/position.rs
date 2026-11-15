//! Board representation, position state, FEN parsing and make/unmake.
//!
//! The layout mirrors `GBoard`, `GData` and the global `Board[]`/`Data[]`
//! arrays.  `Position::data` is the search stack; `cur` is the C++
//! `Current - Data` index.  `do_move`/`undo_move` are exact ports of the
//! templated C++ functions, including the incremental Zobrist keys, PST and
//! material bookkeeping.

use crate::search::{NullReporter, Reporter};
use crate::tables::{tables, Tables, PAWN_HASH_SIZE, PV_HASH_MASK, PV_HASH_SIZE};
use crate::types::*;

pub const HASH_ENTRY_SIZE: usize = 16;

pub struct Hashes(Vec<HashEntry>);

impl Default for Hashes {
    fn default() -> Self {
        Hashes(Vec::new())
    }
}

impl std::ops::Index<usize> for Hashes {
    type Output = HashEntry;
    #[inline(always)]
    fn index(&self, i: usize) -> &HashEntry {
        debug_assert!(i < self.0.len());
        unsafe { self.0.get_unchecked(i) }
    }
}

impl std::ops::IndexMut<usize> for Hashes {
    #[inline(always)]
    fn index_mut(&mut self, i: usize) -> &mut HashEntry {
        debug_assert!(i < self.0.len());
        unsafe { self.0.get_unchecked_mut(i) }
    }
}

impl std::ops::Deref for Hashes {
    type Target = [HashEntry];
    #[inline(always)]
    fn deref(&self) -> &[HashEntry] {
        &self.0
    }
}

impl std::ops::DerefMut for Hashes {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [HashEntry] {
        &mut self.0
    }
}

pub struct PvHashes(Vec<PvEntry>);

impl Default for PvHashes {
    fn default() -> Self {
        PvHashes(Vec::new())
    }
}

impl std::ops::Index<usize> for PvHashes {
    type Output = PvEntry;
    #[inline(always)]
    fn index(&self, i: usize) -> &PvEntry {
        debug_assert!(i < self.0.len());
        unsafe { self.0.get_unchecked(i) }
    }
}

impl std::ops::IndexMut<usize> for PvHashes {
    #[inline(always)]
    fn index_mut(&mut self, i: usize) -> &mut PvEntry {
        debug_assert!(i < self.0.len());
        unsafe { self.0.get_unchecked_mut(i) }
    }
}

impl std::ops::Deref for PvHashes {
    type Target = [PvEntry];
    #[inline(always)]
    fn deref(&self) -> &[PvEntry] {
        &self.0
    }
}

impl std::ops::DerefMut for PvHashes {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [PvEntry] {
        &mut self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HashEntry {
    pub key: u32,
    pub date: u16,
    pub mv: u16,
    pub low: i16,
    pub high: i16,
    pub flags: u16,
    pub low_depth: u8,
    pub high_depth: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PvEntry {
    pub key: u32,
    pub date: u16,
    pub mv: u16,
    pub value: i16,
    pub exclusion: i16,
    pub depth: u8,
    pub ex_depth: u8,
    pub knodes: i32,
    pub ply: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PawnEntry {
    /// Only the low 48 bits are significant, matching the C++ bit-field.
    pub key: u64,
    pub shelter: [u8; 2],
    pub passer: [u8; 2],
    pub draw: [u8; 2],
    pub score: i32,
}

impl PawnEntry {
    #[inline(always)]
    pub fn key48(&self) -> u64 {
        self.key & 0x0000_FFFF_FFFF_FFFF
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reference {
    pub refs: [u16; 2],
    pub check_refs: [u16; 2],
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub num_threads: usize,
    pub multi_pv: usize,
    pub syzygy_probe_depth: usize,
    pub hash_size: usize,
    pub parent_pid: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            num_threads: 1,
            multi_pv: 1,
            syzygy_probe_depth: 1,
            hash_size: 128 * (1 << 20),
            parent_pid: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bitboards(pub [u64; 16]);

impl Default for Bitboards {
    fn default() -> Self {
        Bitboards([0; 16])
    }
}

impl std::ops::Index<usize> for Bitboards {
    type Output = u64;
    #[inline(always)]
    fn index(&self, i: usize) -> &u64 {
        debug_assert!(i < 16);
        unsafe { self.0.get_unchecked(i) }
    }
}

impl std::ops::IndexMut<usize> for Bitboards {
    #[inline(always)]
    fn index_mut(&mut self, i: usize) -> &mut u64 {
        debug_assert!(i < 16);
        unsafe { self.0.get_unchecked_mut(i) }
    }
}

impl std::ops::Deref for Bitboards {
    type Target = [u64];
    #[inline(always)]
    fn deref(&self) -> &[u64] {
        &self.0
    }
}

impl std::ops::DerefMut for Bitboards {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [u64] {
        &mut self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Squares(pub [u8; 64]);

impl Default for Squares {
    fn default() -> Self {
        Squares([0; 64])
    }
}

impl std::ops::Index<usize> for Squares {
    type Output = u8;
    #[inline(always)]
    fn index(&self, i: usize) -> &u8 {
        debug_assert!(i < 64);
        unsafe { self.0.get_unchecked(i) }
    }
}

impl std::ops::IndexMut<usize> for Squares {
    #[inline(always)]
    fn index_mut(&mut self, i: usize) -> &mut u8 {
        debug_assert!(i < 64);
        unsafe { self.0.get_unchecked_mut(i) }
    }
}

impl std::ops::Deref for Squares {
    type Target = [u8];
    #[inline(always)]
    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl std::ops::DerefMut for Squares {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Board {
    pub bb: Bitboards,
    pub square: Squares,
}

impl Default for Board {
    fn default() -> Self {
        Board {
            bb: Bitboards::default(),
            square: Squares::default(),
        }
    }
}

impl Board {
    #[inline(always)]
    pub fn clear(&mut self) {
        self.bb = Bitboards::default();
        self.square = Squares::default();
    }

    #[inline(always)]
    pub fn set(&mut self, s: Square, piece: Piece) {
        self.square[s] = piece as u8;
        if piece != 0 {
            self.bb[piece] |= bit(s);
            self.bb[piece & 1] |= bit(s);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    pub key: u64,
    pub pawn_key: u64,
    pub eval_key: u64,
    pub att: [u64; 2],
    pub patt: [u64; 2],
    pub passer: u64,
    pub xray: [u64; 2],
    pub pin: [u64; 2],
    pub threat: u64,
    pub mask: u64,
    pub turn: u8,
    pub castle_flags: u8,
    pub ply: u8,
    pub ep_square: u8,
    pub capture: u8,
    pub gen_flags: u8,
    pub piece: u8,
    pub stage: u8,
    pub mul: u8,
    pub dummy: u8,
    pub score: i16,
    pub mv: u16,
    pub killer: [u16; 3],
    pub refs: [u16; 2],
    pub best: i32,
    pub material: i32,
    pub pst: i32,
    pub margin: i32,
    pub start: usize,
    pub current: usize,
}

impl Default for State {
    fn default() -> Self {
        State {
            key: 0,
            pawn_key: 0,
            eval_key: 0,
            att: [0; 2],
            patt: [0; 2],
            passer: 0,
            xray: [0; 2],
            pin: [0; 2],
            threat: 0,
            mask: 0,
            turn: 0,
            castle_flags: 0,
            ply: 0,
            ep_square: 0,
            capture: 0,
            gen_flags: 0,
            piece: 0,
            stage: 0,
            mul: 0,
            dummy: 0,
            score: 0,
            mv: 0,
            killer: [0; 3],
            refs: [0; 2],
            best: 0,
            material: 0,
            pst: 0,
            margin: 0,
            start: 0,
            current: 0,
        }
    }
}

#[derive(Clone)]
pub struct MoveLists([[i32; MAX_MOVES]; MAX_PLY]);

impl Default for MoveLists {
    fn default() -> Self {
        MoveLists([[0; MAX_MOVES]; MAX_PLY])
    }
}

impl std::ops::Index<usize> for MoveLists {
    type Output = [i32; MAX_MOVES];
    #[inline(always)]
    fn index(&self, i: usize) -> &[i32; MAX_MOVES] {
        debug_assert!(i < MAX_PLY);
        unsafe { self.0.get_unchecked(i) }
    }
}

impl std::ops::IndexMut<usize> for MoveLists {
    #[inline(always)]
    fn index_mut(&mut self, i: usize) -> &mut [i32; MAX_MOVES] {
        debug_assert!(i < MAX_PLY);
        unsafe { self.0.get_unchecked_mut(i) }
    }
}

impl std::ops::Deref for MoveLists {
    type Target = [[i32; MAX_MOVES]];
    #[inline(always)]
    fn deref(&self) -> &[[i32; MAX_MOVES]] {
        &self.0
    }
}

impl std::ops::DerefMut for MoveLists {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [[i32; MAX_MOVES]] {
        &mut self.0
    }
}

#[derive(Clone, Debug)]
pub struct Data([State; MAX_PLY]);

impl Default for Data {
    fn default() -> Self {
        Data([State::default(); MAX_PLY])
    }
}

impl std::ops::Index<usize> for Data {
    type Output = State;
    #[inline(always)]
    fn index(&self, i: usize) -> &State {
        debug_assert!(i < MAX_PLY);
        unsafe { self.0.get_unchecked(i) }
    }
}

impl std::ops::IndexMut<usize> for Data {
    #[inline(always)]
    fn index_mut(&mut self, i: usize) -> &mut State {
        debug_assert!(i < MAX_PLY);
        unsafe { self.0.get_unchecked_mut(i) }
    }
}

impl std::ops::Deref for Data {
    type Target = [State];
    #[inline(always)]
    fn deref(&self) -> &[State] {
        &self.0
    }
}

impl std::ops::DerefMut for Data {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [State] {
        &mut self.0
    }
}

pub struct Position {
    pub board: Board,
    pub data: Data,
    pub cur: usize,
    pub stack: [u64; STACK_SIZE],
    pub sp: usize,
    pub nodes: u64,
    pub tb_hits: u64,
    pub root_depth: u32,
}

impl Default for Position {
    fn default() -> Self {
        Position {
            board: Board::default(),
            data: Data::default(),
            cur: 0,
            stack: [0; STACK_SIZE],
            sp: 0,
            nodes: 0,
            tb_hits: 0,
            root_depth: 1,
        }
    }
}

impl Position {
    #[inline(always)]
    pub fn cur(&self) -> &State {
        &self.data[self.cur]
    }

    #[inline(always)]
    pub fn cur_mut(&mut self) -> &mut State {
        &mut self.data[self.cur]
    }

    #[inline(always)]
    pub fn turn(&self) -> Color {
        self.data[self.cur].turn as Color
    }
}

/// The complete engine state: immutable tables, position stack, hash tables
/// and search bookkeeping.
pub struct Engine {
    pub tables: &'static Tables,
    pub pos: Position,
    pub pawn_hash: Vec<PawnEntry>,
    pub hash: Hashes,
    pub pv_hash: PvHashes,
    pub hash_mask: usize,
    pub pv_hash_mask: usize,
    pub history: [i16; 16 * 64],
    pub delta: [i16; 16 * 4096],
    pub reference: [Reference; 16 * 64],
    pub root_list: [i32; 256],
    pub move_lists: MoveLists,
    pub multi_pv: [i32; 256],
    pub settings: Settings,
    pub shared: crate::search::SharedInfo,
    pub stats: crate::search::SearchStats,
    pub si: crate::search::SearchInfo,
    pub base_si: crate::search::SearchInfo,
    pub last_depth: i32,
    pub last_time: i64,
    pub last_value: i32,
    pub last_exact_value: i32,
    pub last_speed: i64,
    pub prev_move: i32,
    pub inst_cnt: i32,
    pub pv_hashing: i32,
    pub search_moves: i32,
    pub previous: i32,
    pub aspiration: i32,
    pub info_time: i64,
    pub reporter: Box<dyn Reporter>,
    pub control: crate::search::SearchControl,
    pub save_board: Board,
    pub save_state: State,
    pub save_sp: usize,
}

impl Engine {
    /// Create an engine with the original C++ default hash size (128 MB).
    pub fn new() -> Engine {
        Engine::with_hash_mb(128)
    }

    /// Create an engine with a hash table of `mb` megabytes.
    pub fn with_hash_mb(mb: u32) -> Engine {
        let bytes = hash_size_bytes(mb as u64);
        let entries = bytes / HASH_ENTRY_SIZE;
        debug_assert!(entries.is_power_of_two());
        let mut e = Engine {
            tables: tables(),
            pos: Position::default(),
            pawn_hash: vec![PawnEntry::default(); PAWN_HASH_SIZE],
            hash: Hashes::default(),
            pv_hash: PvHashes::default(),
            hash_mask: entries - 4,
            pv_hash_mask: PV_HASH_MASK,
            history: [0; 16 * 64],
            delta: [0; 16 * 4096],
            reference: [Reference::default(); 16 * 64],
            root_list: [0; 256],
            move_lists: MoveLists::default(),
            multi_pv: [0; 256],
            settings: Settings {
                hash_size: bytes,
                ..Settings::default()
            },
            shared: crate::search::SharedInfo::default(),
            stats: crate::search::SearchStats::default(),
            si: crate::search::SearchInfo::default(),
            base_si: crate::search::SearchInfo::default(),
            last_depth: 128,
            last_time: 0,
            last_value: 0,
            last_exact_value: 0,
            last_speed: 0,
            prev_move: 0,
            inst_cnt: 0,
            pv_hashing: 1,
            search_moves: 0,
            previous: 0,
            aspiration: 1,
            info_time: 0,
            reporter: Box::new(NullReporter),
            control: crate::search::SearchControl::default(),
            save_board: Board::default(),
            save_state: State::default(),
            save_sp: 0,
        };
        e.init_search(true);
        e
    }

    /// Resize the transposition table; `mb` follows the UCI Hash option.
    pub fn resize_hash(&mut self, mb: u64) {
        let bytes = hash_size_bytes(mb);
        self.settings.hash_size = bytes;
        self.init_search(true);
    }

    pub fn init_search(&mut self, clear_hash: bool) {
        self.history.fill(0x0101);
        self.delta.fill(0);
        self.reference.fill(Reference::default());
        for s in self.pos.data.iter_mut().skip(1).take(127) {
            *s = State::default();
        }
        if clear_hash {
            self.shared.date = 1;
            self.hash.fill(HashEntry::default());
            self.pv_hash.fill(PvEntry::default());
        }
        let bytes = self.settings.hash_size;
        let entries = bytes / HASH_ENTRY_SIZE;
        debug_assert!(entries.is_power_of_two());
        if self.hash.len() != entries {
            self.hash = Hashes(vec![HashEntry::default(); entries]);
        }
        self.hash_mask = entries - 4;
        self.pv_hash_mask = PV_HASH_MASK;
        if self.pv_hash.len() != PV_HASH_SIZE {
            self.pv_hash = PvHashes(vec![PvEntry::default(); PV_HASH_SIZE]);
        }
        self.get_board("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        self.last_time = 0;
        self.last_value = 0;
        self.last_exact_value = 0;
        self.inst_cnt = 0;
        self.last_speed = 0;
        self.search_moves = 0;
        self.last_depth = 128;
        self.si = crate::search::SearchInfo::default();
        self.base_si = crate::search::SearchInfo::default();
        self.previous = 0;
        self.prev_move = 0;
        self.info_time = 0;
        self.pos.nodes = 0;
        self.pos.tb_hits = 0;
    }

    pub fn setup_board(&mut self) {
        let t = &self.tables;
        self.pos.sp = 0;
        self.shared.date = self.shared.date.wrapping_add(1);
        if self.shared.date > 0x8000 {
            self.shared.date = 2;
            for e in self.hash.iter_mut() {
                e.date = 1;
            }
            for e in self.pv_hash.iter_mut() {
                e.date = 1;
            }
        }
        {
            let st = &mut self.pos.data[0];
            st.material = 0;
            st.pst = 0;
            st.key = t.piece_key[0][0];
            if st.turn != 0 {
                st.key ^= t.turn_key;
            }
            st.key ^= t.castle_key[st.castle_flags as usize];
            if st.ep_square != 0 {
                st.key ^= t.ep_key[file(st.ep_square as usize)];
            }
            st.pawn_key = 0;
            st.pawn_key ^= t.castle_key[st.castle_flags as usize];
        }
        for i in 0..16 {
            self.pos.board.bb[i] = 0;
        }
        let mut occ = 0u64;
        for i in 0..64 {
            let piece = self.pos.board.square[i] as usize;
            if piece != 0 {
                self.pos.board.bb[piece] |= bit(i);
                self.pos.board.bb[piece & 1] |= bit(i);
                occ |= bit(i);
                let st = &mut self.pos.data[0];
                st.key ^= t.piece_key[piece][i];
                if piece < WHITE_KNIGHT {
                    st.pawn_key ^= t.piece_key[piece][i];
                }
                if piece < WHITE_KING {
                    st.material += MAT_CODE[piece];
                } else {
                    st.pawn_key ^= t.piece_key[piece][i];
                }
                st.pst += t.pst[(piece << 6) | i];
            }
        }
        let _ = occ;
        let st = &mut self.pos.data[0];
        if popcount(self.pos.board.bb[WHITE_KNIGHT]) > 2
            || popcount(self.pos.board.bb[WHITE_LIGHT]) > 1
            || popcount(self.pos.board.bb[WHITE_DARK]) > 1
            || popcount(self.pos.board.bb[WHITE_ROOK]) > 2
            || popcount(self.pos.board.bb[WHITE_QUEEN]) > 2
        {
            st.material |= FLAG_UNUSUAL_MATERIAL;
        }
        if popcount(self.pos.board.bb[BLACK_KNIGHT]) > 2
            || popcount(self.pos.board.bb[BLACK_LIGHT]) > 1
            || popcount(self.pos.board.bb[BLACK_DARK]) > 1
            || popcount(self.pos.board.bb[BLACK_ROOK]) > 2
            || popcount(self.pos.board.bb[BLACK_QUEEN]) > 2
        {
            st.material |= FLAG_UNUSUAL_MATERIAL;
        }
        st.capture = 0;
        st.killer[1] = 0;
        st.killer[2] = 0;
        st.ply = 0;
        self.pos.cur = 0;
        self.pos.sp = 0;
        self.pos.stack[0] = st.key;
    }

    /// Parse a FEN.  Returns the remainder of the string after the six FEN
    /// fields (used for a trailing `moves ...` list).
    /// Apply a move in UCI long algebraic notation to the current position.
    /// Returns `false` if the move is not legal.
    pub fn play_uci_move(&mut self, text: &str) -> bool {
        if text.len() < 4 {
            return false;
        }
        let from = match crate::types::parse_square(&text[0..2]) {
            Some(s) => s,
            None => return false,
        };
        let to = match crate::types::parse_square(&text[2..4]) {
            Some(s) => s,
            None => return false,
        };
        if from == to {
            return false;
        }
        let me = self.pos.cur().turn as Color;
        let piece = self.pos.board.square[from] as usize;
        let capture = self.pos.board.square[to] as usize;
        // Basic sanity checks.  The C++ front end trusts the GUI and applies
        // the move directly; we do the same, but reject obviously broken input
        // instead of corrupting the board.
        if piece == 0 || (piece & 1) != me as usize {
            return false;
        }
        if capture != 0 && (capture & 1) == (piece & 1) {
            return false;
        }

        // Reconstruct exactly the flags that C++ `move_from_string` derives
        // from the board state.  In particular, do not require the move to be
        // present in a generated legal-move list: the old validation path
        // could omit legal moves, and the UCI layer must accept every GUI
        // move or the engine will desynchronise from the board.
        let mut mv: i32 = ((from as i32) << 6) | (to as i32);
        if piece >= WHITE_KING && (to as i32 - from as i32).abs() == 2 {
            mv |= FLAG_CASTLING;
        }
        if self.pos.data[self.pos.cur].ep_square != 0
            && to == self.pos.data[self.pos.cur].ep_square as usize
            && piece == ipawn(me)
        {
            mv |= FLAG_EP;
        }
        if text.len() >= 5 {
            mv |= match text.as_bytes()[4].to_ascii_lowercase() {
                b'q' => FLAG_P_QUEEN,
                b'r' => FLAG_P_ROOK,
                b'n' => FLAG_P_KNIGHT,
                b'b' => {
                    if ((to ^ (to >> 3)) & 1) != 0 {
                        FLAG_P_LIGHT
                    } else {
                        FLAG_P_DARK
                    }
                }
                _ => 0,
            };
        }

        self.do_move(me, mv);
        // UCI positions are always rooted at Data[0].  The C++ front end
        // copies the resulting position back to Data[0] after every move and
        // truncates the repetition stack to the current halfmove clock.
        let cur = self.pos.cur;
        self.pos.data[0] = self.pos.data[cur];
        self.pos.cur = 0;
        let ply = self.pos.data[0].ply as usize;
        if self.pos.sp >= ply {
            self.pos
                .stack
                .copy_within(self.pos.sp - ply..=self.pos.sp, 0);
            self.pos.sp = ply;
        }
        self.shared.root_depth = self.shared.root_depth.wrapping_add(1);
        true
    }

    pub fn get_board(&mut self, fen: &str) -> String {
        self.pos.cur = 0;
        self.pos.board.clear();
        self.pos.data[0] = State::default();
        let fields: Vec<&str> = fen.split_whitespace().collect();
        let placement = fields.first().copied().unwrap_or("");

        let mut rank = 7i32;
        let mut file = 0i32;
        for c in placement.bytes() {
            match c {
                b'/' => {
                    rank -= 1;
                    file = 0;
                }
                b'1'..=b'8' => file += (c - b'0') as i32,
                _ => {
                    if (0..8).contains(&rank) && (0..8).contains(&file) {
                        let s = sq(file as usize, rank as usize);
                        let piece = piece_from_char(c, s);
                        self.pos.board.square[s] = piece as u8;
                    }
                    file += 1;
                }
            }
        }

        // Side to move, castling, en passant, halfmove and fullmove.
        let side = fields.get(1).copied().unwrap_or("w");
        let castle = fields.get(2).copied().unwrap_or("-");
        let ep = fields.get(3).copied().unwrap_or("-");
        let halfmove = fields
            .get(4)
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(0);
        let fullmove = fields
            .get(5)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);

        self.pos.data[0].turn = if side.starts_with('b') { 1 } else { 0 };
        let mut flags = 0u8;
        if castle.contains('K') {
            flags |= CAN_CASTLE_OO;
        }
        if castle.contains('Q') {
            flags |= CAN_CASTLE_OOO;
        }
        if castle.contains('k') {
            flags |= CAN_CASTLE_oo;
        }
        if castle.contains('q') {
            flags |= CAN_CASTLE_ooo;
        }
        self.pos.data[0].castle_flags = flags;

        let mut ep_square = 0u8;
        if ep.len() >= 2 && ep != "-" {
            if let Some(s) = parse_square(ep) {
                let turn = self.pos.data[0].turn;
                let j = s ^ 8;
                // Reproduce the C++ validation exactly.
                let valid = {
                    let empty = self.pos.board.square[s] == 0;
                    let enemy = self.pos.board.square[j] == (3 - turn);
                    let adjacent = self.pos.board.square[j - 1] == (turn + 2)
                        || self.pos.board.square[j + 1] == (turn + 2);
                    empty && enemy && adjacent
                };
                if valid {
                    ep_square = s as u8;
                }
            }
        }
        self.pos.data[0].ep_square = ep_square;
        self.pos.data[0].ply = halfmove;
        self.shared.root_depth = fullmove;
        self.setup_board();
        fields.get(6..).map(|r| r.join(" ")).unwrap_or_default()
    }

    /// FEN of the current search state (including side to move, castling and
    /// en-passant square).  Used by search tracing.
    pub fn current_fen(&self) -> String {
        let st = &self.pos.data[self.pos.cur];
        let mut out = String::new();
        for r in (0..8).rev() {
            let mut empty = 0;
            for f in 0..8 {
                let p = self.pos.board.square[sq(f, r)] as usize;
                if p == 0 {
                    empty += 1;
                } else {
                    if empty > 0 {
                        out.push((b'0' + empty as u8) as char);
                        empty = 0;
                    }
                    out.push(piece_char(p));
                }
            }
            if empty > 0 {
                out.push((b'0' + empty as u8) as char);
            }
            if r > 0 {
                out.push('/');
            }
        }
        out.push(' ');
        out.push(if st.turn == 0 { 'w' } else { 'b' });
        out.push(' ');
        let mut c = String::new();
        if st.castle_flags & CAN_CASTLE_OO != 0 {
            c.push('K');
        }
        if st.castle_flags & CAN_CASTLE_OOO != 0 {
            c.push('Q');
        }
        if st.castle_flags & CAN_CASTLE_oo != 0 {
            c.push('k');
        }
        if st.castle_flags & CAN_CASTLE_ooo != 0 {
            c.push('q');
        }
        if c.is_empty() {
            c.push('-');
        }
        out.push_str(&c);
        out.push(' ');
        if st.ep_square != 0 {
            out.push_str(&square_to_string(st.ep_square as usize));
        } else {
            out.push('-');
        }
        out.push_str(&format!(" {} 1", st.ply));
        out
    }

    /// Parse a FEN and return the board as a FEN placement string (used by
    /// tests and debugging).
    pub fn to_fen(&self) -> String {
        let mut out = String::new();
        for r in (0..8).rev() {
            let mut empty = 0;
            for f in 0..8 {
                let p = self.pos.board.square[sq(f, r)] as usize;
                if p == 0 {
                    empty += 1;
                } else {
                    if empty > 0 {
                        out.push((b'0' + empty as u8) as char);
                        empty = 0;
                    }
                    out.push(piece_char(p));
                }
            }
            if empty > 0 {
                out.push((b'0' + empty as u8) as char);
            }
            if r > 0 {
                out.push('/');
            }
        }
        let st = &self.pos.data[0];
        out.push(' ');
        out.push(if st.turn == 0 { 'w' } else { 'b' });
        out.push(' ');
        let mut c = String::new();
        if st.castle_flags & CAN_CASTLE_OO != 0 {
            c.push('K');
        }
        if st.castle_flags & CAN_CASTLE_OOO != 0 {
            c.push('Q');
        }
        if st.castle_flags & CAN_CASTLE_oo != 0 {
            c.push('k');
        }
        if st.castle_flags & CAN_CASTLE_ooo != 0 {
            c.push('q');
        }
        if c.is_empty() {
            c.push('-');
        }
        out.push_str(&c);
        out.push(' ');
        if st.ep_square != 0 {
            out.push_str(&square_to_string(st.ep_square as usize));
        } else {
            out.push('-');
        }
        out.push_str(" 0 1");
        out
    }

    #[inline(always)]
    pub fn piece(&self, me: Color) -> u64 {
        self.pos.board.bb[me]
    }
    #[inline(always)]
    pub fn pawn(&self, me: Color) -> u64 {
        self.pos.board.bb[WHITE_PAWN | me]
    }
    #[inline(always)]
    pub fn knight(&self, me: Color) -> u64 {
        self.pos.board.bb[WHITE_KNIGHT | me]
    }
    #[inline(always)]
    pub fn bishop(&self, me: Color) -> u64 {
        self.pos.board.bb[WHITE_LIGHT | me] | self.pos.board.bb[WHITE_DARK | me]
    }
    #[inline(always)]
    pub fn rook(&self, me: Color) -> u64 {
        self.pos.board.bb[WHITE_ROOK | me]
    }
    #[inline(always)]
    pub fn queen(&self, me: Color) -> u64 {
        self.pos.board.bb[WHITE_QUEEN | me]
    }
    #[inline(always)]
    pub fn king(&self, me: Color) -> u64 {
        self.pos.board.bb[WHITE_KING | me]
    }
    #[inline(always)]
    pub fn non_pawn(&self, me: Color) -> u64 {
        self.piece(me) ^ self.pawn(me)
    }
    #[inline(always)]
    pub fn non_pawn_king(&self, me: Color) -> u64 {
        self.non_pawn(me) ^ self.king(me)
    }
    #[inline(always)]
    pub fn bslider(&self, me: Color) -> u64 {
        self.bishop(me) | self.queen(me)
    }
    #[inline(always)]
    pub fn rslider(&self, me: Color) -> u64 {
        self.rook(me) | self.queen(me)
    }
    #[inline(always)]
    pub fn major(&self, me: Color) -> u64 {
        self.rslider(me)
    }
    #[inline(always)]
    pub fn minor(&self, me: Color) -> u64 {
        self.knight(me) | self.bishop(me)
    }
    #[inline(always)]
    pub fn slider(&self, me: Color) -> u64 {
        self.bslider(me) | self.rslider(me)
    }
    #[inline(always)]
    pub fn piece_all(&self) -> u64 {
        self.piece(WHITE) | self.piece(BLACK)
    }
    #[inline(always)]
    pub fn slider_all(&self) -> u64 {
        self.slider(WHITE) | self.slider(BLACK)
    }
    #[inline(always)]
    pub fn pawn_all(&self) -> u64 {
        self.pawn(WHITE) | self.pawn(BLACK)
    }
    #[inline(always)]
    pub fn non_pawn_king_all(&self) -> u64 {
        self.non_pawn_king(WHITE) | self.non_pawn_king(BLACK)
    }
    #[inline(always)]
    pub fn king_pos(&self, me: Color) -> Square {
        lsb(self.king(me))
    }
    #[inline(always)]
    pub fn square(&self, s: Square) -> Piece {
        self.pos.board.square[s] as Piece
    }

    #[inline(always)]
    pub fn shift_nw(target: u64) -> u64 {
        (target & !(FILE_A | LINE_7)) << 7
    }
    #[inline(always)]
    pub fn shift_ne(target: u64) -> u64 {
        (target & !(FILE_H | LINE_7)) << 9
    }
    #[inline(always)]
    pub fn shift_se(target: u64) -> u64 {
        (target & !(FILE_H | LINE_0)) >> 7
    }
    #[inline(always)]
    pub fn shift_sw(target: u64) -> u64 {
        (target & !(FILE_A | LINE_0)) >> 9
    }
    #[inline(always)]
    pub fn shift_w(me: Color, target: u64) -> u64 {
        if me == BLACK {
            Self::shift_sw(target)
        } else {
            Self::shift_nw(target)
        }
    }
    #[inline(always)]
    pub fn shift_e(me: Color, target: u64) -> u64 {
        if me == BLACK {
            Self::shift_se(target)
        } else {
            Self::shift_ne(target)
        }
    }
    #[inline(always)]
    pub fn shift(me: Color, target: u64) -> u64 {
        if me == BLACK {
            target >> 8
        } else {
            target << 8
        }
    }
    #[inline(always)]
    pub fn push(me: Color) -> i32 {
        if me == BLACK {
            -8
        } else {
            8
        }
    }
    #[inline(always)]
    pub fn push_w(me: Color) -> i32 {
        if me == BLACK {
            -9
        } else {
            7
        }
    }
    #[inline(always)]
    pub fn push_e(me: Color) -> i32 {
        if me == BLACK {
            -7
        } else {
            9
        }
    }
    #[inline(always)]
    pub fn dir(me: Color) -> i32 {
        if me == BLACK {
            -1
        } else {
            1
        }
    }
    #[inline(always)]
    pub fn is_greater(me: Color, x: Square, y: Square) -> bool {
        if me == BLACK {
            x < y
        } else {
            x > y
        }
    }

    #[inline(always)]
    pub fn line(&self, me: Color, n: usize) -> u64 {
        if me == BLACK {
            self.tables.line[7 - n]
        } else {
            self.tables.line[n]
        }
    }

    /// `do_move<me>`
    #[inline(always)]
    pub fn do_move(&mut self, me: Color, mv: Move) {
        if me == WHITE {
            self.do_move_c::<true>(mv);
        } else {
            self.do_move_c::<false>(mv);
        }
    }

    fn do_move_c<const ME: bool>(&mut self, mv: Move) {
        let me: Color = if ME { WHITE } else { BLACK };
        let t = &self.tables;
        let cur_idx = self.pos.cur;
        let current = self.pos.data[cur_idx];
        let mut next = self.pos.data[cur_idx + 1];
        next.ep_square = 0;
        let to = move_to(mv);
        let capture = self.pos.board.square[to] as usize;
        let mut finish_via_capture = false;

        if capture == 0 {
            next.capture = 0;
        } else {
            let from = move_from(mv);
            let mut piece = self.pos.board.square[from] as usize;
            let oppc = opp(me);
            next.turn = oppc as u8;
            next.capture = capture as u8;
            self.pos.board.square[from] = 0;
            self.pos.board.square[to] = piece as u8;
            next.piece = piece as u8;
            let mask_from = bit(from);
            let mask_to = bit(to);
            self.pos.board.bb[piece] ^= mask_from;
            self.pos.board.bb[me] ^= mask_from;
            self.pos.board.bb[capture] ^= mask_to;
            self.pos.board.bb[oppc] ^= mask_to;
            self.pos.board.bb[piece] |= mask_to;
            self.pos.board.bb[me] |= mask_to;
            next.castle_flags =
                current.castle_flags & t.update_castling[to] & t.update_castling[from];
            next.pst = current.pst + t.pst[(piece << 6) | to]
                - t.pst[(piece << 6) | from]
                - t.pst[(capture << 6) | to];
            next.key = current.key
                ^ t.piece_key[piece][from]
                ^ t.piece_key[piece][to]
                ^ t.piece_key[capture][to]
                ^ t.castle_key[current.castle_flags as usize]
                ^ t.castle_key[next.castle_flags as usize];
            if capture != ipawn(oppc) {
                next.pawn_key = current.pawn_key
                    ^ t.castle_key[current.castle_flags as usize]
                    ^ t.castle_key[next.castle_flags as usize];
            } else {
                next.pawn_key = current.pawn_key
                    ^ t.piece_key[ipawn(oppc)][to]
                    ^ t.castle_key[current.castle_flags as usize]
                    ^ t.castle_key[next.castle_flags as usize];
            }
            next.material = current.material - MAT_CODE[capture];
            if (current.material & FLAG_UNUSUAL_MATERIAL) != 0 && capture >= WHITE_KNIGHT {
                if popcount(self.pos.board.bb[WHITE_QUEEN]) <= 2
                    && popcount(self.pos.board.bb[BLACK_QUEEN]) <= 2
                    && popcount(self.pos.board.bb[WHITE_LIGHT]) <= 1
                    && popcount(self.pos.board.bb[BLACK_LIGHT]) <= 1
                    && popcount(self.pos.board.bb[WHITE_KNIGHT]) <= 2
                    && popcount(self.pos.board.bb[BLACK_KNIGHT]) <= 2
                    && popcount(self.pos.board.bb[WHITE_ROOK]) <= 2
                    && popcount(self.pos.board.bb[BLACK_ROOK]) <= 2
                {
                    next.material ^= FLAG_UNUSUAL_MATERIAL;
                }
            }
            if piece == ipawn(me) {
                next.pawn_key ^= t.piece_key[ipawn(me)][from] ^ t.piece_key[piece][to];
                if is_promotion(mv) {
                    piece = promotion_piece(mv, me);
                    self.pos.board.square[to] = piece as u8;
                    next.material += MAT_CODE[piece] - MAT_CODE[ipawn(me)];
                    if piece < WHITE_ROOK {
                        if piece >= WHITE_LIGHT && self.pos.board.bb[piece] != 0 {
                            next.material |= FLAG_UNUSUAL_MATERIAL;
                        }
                        if multiple(self.pos.board.bb[piece]) {
                            next.material |= FLAG_UNUSUAL_MATERIAL;
                        }
                    } else if multiple(self.pos.board.bb[piece]) {
                        next.material |= FLAG_UNUSUAL_MATERIAL;
                    }
                    self.pos.board.bb[WHITE_PAWN | me] ^= mask_to;
                    self.pos.board.bb[piece] |= mask_to;
                    next.pst += t.pst[(piece << 6) | to] - t.pst[(ipawn(me) << 6) | to];
                    next.key ^= t.piece_key[piece][to] ^ t.piece_key[ipawn(me)][to];
                    next.pawn_key ^= t.piece_key[ipawn(me)][to];
                }
            } else if piece >= WHITE_KING {
                next.pawn_key ^= t.piece_key[piece][from] ^ t.piece_key[piece][to];
            }
            if current.ep_square != 0 {
                next.key ^= t.ep_key[file(current.ep_square as usize)];
            }
            next.turn = (current.turn ^ 1) as u8;
            next.key ^= t.turn_key;
            next.ply = 0;
            finish_via_capture = true;
        }

        if !finish_via_capture {
            let from = move_from(mv);
            next.ply = current.ply.wrapping_add(1);
            let mut piece = self.pos.board.square[from] as usize;
            self.pos.board.square[from] = 0;
            self.pos.board.square[to] = piece as u8;
            next.piece = piece as u8;
            let mask_from = bit(from);
            let mask_to = bit(to);
            self.pos.board.bb[piece] ^= mask_from;
            self.pos.board.bb[me] ^= mask_from;
            self.pos.board.bb[piece] |= mask_to;
            self.pos.board.bb[me] |= mask_to;
            next.castle_flags =
                current.castle_flags & t.update_castling[to] & t.update_castling[from];
            next.pst = current.pst + t.pst[(piece << 6) | to] - t.pst[(piece << 6) | from];
            next.key = current.key
                ^ t.piece_key[piece][to]
                ^ t.piece_key[piece][from]
                ^ t.castle_key[current.castle_flags as usize]
                ^ t.castle_key[next.castle_flags as usize];
            next.material = current.material;
            if piece == ipawn(me) {
                next.ply = 0;
                next.pawn_key = current.pawn_key
                    ^ t.piece_key[ipawn(me)][to]
                    ^ t.piece_key[ipawn(me)][from]
                    ^ t.castle_key[current.castle_flags as usize]
                    ^ t.castle_key[next.castle_flags as usize];
                if is_ep(mv) {
                    let cap_sq = to ^ 8;
                    self.pos.board.square[cap_sq] = 0;
                    let u = bit(cap_sq);
                    next.key ^= t.piece_key[ipawn(opp(me))][cap_sq];
                    next.pawn_key ^= t.piece_key[ipawn(opp(me))][cap_sq];
                    next.pst -= t.pst[(ipawn(opp(me)) << 6) | cap_sq];
                    self.pos.board.bb[WHITE_PAWN | opp(me)] &= !u;
                    self.pos.board.bb[opp(me)] &= !u;
                    next.material -= MAT_CODE[ipawn(opp(me))];
                } else if is_promotion(mv) {
                    piece = promotion_piece(mv, me);
                    self.pos.board.square[to] = piece as u8;
                    next.material += MAT_CODE[piece] - MAT_CODE[ipawn(me)];
                    if piece < WHITE_ROOK {
                        if piece >= WHITE_LIGHT && self.pos.board.bb[piece] != 0 {
                            next.material |= FLAG_UNUSUAL_MATERIAL;
                        }
                        if multiple(self.pos.board.bb[piece]) {
                            next.material |= FLAG_UNUSUAL_MATERIAL;
                        }
                    } else if multiple(self.pos.board.bb[piece]) {
                        next.material |= FLAG_UNUSUAL_MATERIAL;
                    }
                    self.pos.board.bb[WHITE_PAWN | me] ^= mask_to;
                    self.pos.board.bb[piece] |= mask_to;
                    next.pst += t.pst[(piece << 6) | to] - t.pst[(ipawn(me) << 6) | to];
                    next.key ^= t.piece_key[piece][to] ^ t.piece_key[ipawn(me)][to];
                    next.pawn_key ^= t.piece_key[ipawn(me)][to];
                } else if (to ^ from) == 16 {
                    if t.p_att[me][(to + from) >> 1] & self.pawn(opp(me)) != 0 {
                        next.ep_square = ((to + from) >> 1) as u8;
                        next.key ^= t.ep_key[file(next.ep_square as usize)];
                    }
                }
            } else {
                if piece < WHITE_KING {
                    next.pawn_key = current.pawn_key
                        ^ t.castle_key[current.castle_flags as usize]
                        ^ t.castle_key[next.castle_flags as usize];
                } else {
                    next.pawn_key = current.pawn_key
                        ^ t.piece_key[piece][to]
                        ^ t.piece_key[piece][from]
                        ^ t.castle_key[current.castle_flags as usize]
                        ^ t.castle_key[next.castle_flags as usize];
                }
                if is_castling(mv) {
                    let (rold, rnew) = match to {
                        6 => (7, 5),
                        2 => (0, 3),
                        62 => (63, 61),
                        58 => (56, 59),
                        _ => (0, 0),
                    };
                    self.pos.board.square[rold] = 0;
                    self.pos.board.square[rnew] = irook(me) as u8;
                    self.pos.board.bb[irook(me)] ^= bit(rold);
                    self.pos.board.bb[me] ^= bit(rold);
                    self.pos.board.bb[irook(me)] |= bit(rnew);
                    self.pos.board.bb[me] |= bit(rnew);
                    next.pst += t.pst[(irook(me) << 6) | rnew] - t.pst[(irook(me) << 6) | rold];
                    next.key ^= t.piece_key[irook(me)][rnew] ^ t.piece_key[irook(me)][rold];
                }
            }
            if current.ep_square != 0 {
                next.key ^= t.ep_key[file(current.ep_square as usize)];
            }
            next.turn = opp(me) as u8;
            next.key ^= t.turn_key;
        }

        self.pos.data[cur_idx + 1] = next;
        self.pos.sp += 1;
        self.pos.stack[self.pos.sp] = next.key;
        self.pos.data[cur_idx + 1].mv = mv as u16;
        self.pos.data[cur_idx + 1].gen_flags = 0;
        self.pos.cur += 1;
        self.pos.nodes += 1;
    }

    /// `undo_move<me>`
    #[inline(always)]
    pub fn undo_move(&mut self, me: Color, mv: Move) {
        if me == WHITE {
            self.undo_move_c::<true>(mv);
        } else {
            self.undo_move_c::<false>(mv);
        }
    }

    fn undo_move_c<const ME: bool>(&mut self, mv: Move) {
        let me: Color = if ME { WHITE } else { BLACK };
        let to = move_to(mv);
        let from = move_from(mv);
        let cur = self.pos.data[self.pos.cur];
        if is_promotion(mv) {
            let p = self.pos.board.square[to] as usize;
            self.pos.board.bb[p] ^= bit(to);
            let piece = ipawn(me);
            self.pos.board.square[from] = piece as u8;
            self.pos.board.bb[piece] |= bit(from);
            self.pos.board.bb[me] |= bit(from);
            self.pos.board.bb[piece] &= !bit(to);
            self.pos.board.bb[me] ^= bit(to);
        } else {
            let piece = self.pos.board.square[to] as usize;
            self.pos.board.square[from] = piece as u8;
            self.pos.board.bb[piece] |= bit(from);
            self.pos.board.bb[me] |= bit(from);
            self.pos.board.bb[piece] &= !bit(to);
            self.pos.board.bb[me] ^= bit(to);
        }
        self.pos.board.square[to] = cur.capture;
        if cur.capture != 0 {
            let c = cur.capture as usize;
            self.pos.board.bb[c] |= bit(to);
            self.pos.board.bb[opp(me)] |= bit(to);
        } else if is_castling(mv) {
            let (rold, rnew) = match to {
                6 => (7, 5),
                2 => (0, 3),
                62 => (63, 61),
                58 => (56, 59),
                _ => (0, 0),
            };
            self.pos.board.square[rnew] = 0;
            self.pos.board.square[rold] = irook(me) as u8;
            self.pos.board.bb[WHITE_ROOK | me] ^= bit(rnew);
            self.pos.board.bb[me] ^= bit(rnew);
            self.pos.board.bb[WHITE_ROOK | me] |= bit(rold);
            self.pos.board.bb[me] |= bit(rold);
        } else if is_ep(mv) {
            let sq = to ^ 8;
            let piece = ipawn(opp(me));
            self.pos.board.square[sq] = piece as u8;
            self.pos.board.bb[opp(me)] |= bit(sq);
            self.pos.board.bb[WHITE_PAWN | opp(me)] |= bit(sq);
        }
        self.pos.cur -= 1;
        self.pos.sp -= 1;
    }

    /// `do_null()`
    #[inline(always)]
    pub fn do_null(&mut self) {
        let t = &self.tables;
        let cur_idx = self.pos.cur;
        let current = self.pos.data[cur_idx];
        let mut next = self.pos.data[cur_idx + 1];
        next.key = current.key ^ t.turn_key;
        next.pawn_key = current.pawn_key;
        next.eval_key = 0;
        next.turn = current.turn ^ 1;
        next.material = current.material;
        next.pst = current.pst;
        next.ply = 0;
        next.castle_flags = current.castle_flags;
        next.ep_square = 0;
        next.capture = 0;
        if current.ep_square != 0 {
            next.key ^= t.ep_key[file(current.ep_square as usize)];
        }
        next.att[WHITE] = current.att[WHITE];
        next.att[BLACK] = current.att[BLACK];
        next.patt[WHITE] = current.patt[WHITE];
        next.patt[BLACK] = current.patt[BLACK];
        next.xray[WHITE] = current.xray[WHITE];
        next.xray[BLACK] = current.xray[BLACK];
        next.pin[WHITE] = current.pin[WHITE];
        next.pin[BLACK] = current.pin[BLACK];
        next.threat = current.threat;
        next.passer = current.passer;
        next.score = current.score.wrapping_neg();
        next.mv = 0;
        next.gen_flags = 0;
        self.pos.data[cur_idx + 1] = next;
        self.pos.sp += 1;
        self.pos.stack[self.pos.sp] = next.key;
        self.pos.cur += 1;
        self.pos.nodes += 1;
    }

    pub fn undo_null(&mut self) {
        self.pos.cur -= 1;
        self.pos.sp -= 1;
    }

    // -- root snapshots ----------------------------------------------------

    pub fn save_root(&mut self) {
        self.save_board = self.pos.board;
        self.save_state = self.pos.data[0];
        self.save_sp = self.pos.sp;
        self.shared.root_board = self.pos.board;
        self.shared.root_state = self.pos.data[0];
        self.shared.root_sp = self.pos.sp;
        self.shared.root_stack = self.pos.stack;
    }

    pub fn restore_root(&mut self) {
        self.pos.board = self.save_board;
        self.pos.data[0] = self.save_state;
        self.pos.cur = 0;
        self.pos.sp = self.save_sp;
    }

    pub fn restore_shared_root(&mut self) {
        self.pos.board = self.shared.root_board;
        self.pos.data[0] = self.shared.root_state;
        self.pos.cur = 0;
        self.pos.sp = self.shared.root_sp;
        self.pos.stack = self.shared.root_stack;
    }
}

/// `FILE_H` and `LINE_7` helpers used by the shift macros.
pub const FILE_H: u64 = 0x8080_8080_8080_8080;
pub const LINE_7: u64 = 0xFF00_0000_0000_0000;

/// `Hash` option conversion: `HASH_SIZE(n)` from the C++ source.
pub fn hash_size_bytes(n: u64) -> usize {
    if n == 0 {
        1 << 20
    } else {
        let bytes = (n as usize).saturating_mul(1 << 20);
        let msb = (usize::BITS - 1 - bytes.leading_zeros()) as usize;
        1usize << msb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startpos_keys_match_reference() {
        let e = Engine::with_hash_mb(1);
        // Values produced by the C++ reference harness for the start position.
        assert_eq!(e.pos.cur().pawn_key, 0x78746e165ee92e55);
        assert_eq!(e.pos.cur().key, 0x21d09a40dda19091);
        assert_eq!(e.pos.cur().material, 944779);
        assert_eq!(e.pos.cur().pst, 0);
        assert_eq!(e.pos.cur().turn, 0);
    }

    #[test]
    fn kiwipete_keys_match_reference() {
        let mut e = Engine::with_hash_mb(1);
        e.get_board("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
        assert_eq!(e.pos.cur().pawn_key, 0xf7904d3200f0397e);
        assert_eq!(e.pos.cur().key, 0x70537df50de78123);
        assert_eq!(e.pos.cur().material, 944779);
        assert_eq!(
            e.to_fen(),
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"
        );
    }

    #[test]
    fn fen_roundtrip_bishops_and_castling() {
        let mut e = Engine::with_hash_mb(1);
        let fen = "r1bqk2r/pppp1ppp/2n2n2/2b1p3/2B1P3/2N2N2/PPPP1PPP/R1BQK2R w KQkq - 0 1";
        e.get_board(fen);
        assert_eq!(e.to_fen(), fen);
        assert_eq!(
            e.pos.board.bb[WHITE_LIGHT].count_ones() + e.pos.board.bb[WHITE_DARK].count_ones(),
            2
        );
        assert_eq!(
            e.pos.board.bb[BLACK_LIGHT].count_ones() + e.pos.board.bb[BLACK_DARK].count_ones(),
            2
        );
    }

    #[test]
    fn hash_size_formula() {
        assert_eq!(hash_size_bytes(0), 1 << 20);
        assert_eq!(hash_size_bytes(1), 1 << 20);
        assert_eq!(hash_size_bytes(2), 1 << 21);
        assert_eq!(hash_size_bytes(3), 1 << 21);
        assert_eq!(hash_size_bytes(8), 1 << 23);
        assert_eq!(hash_size_bytes(128), 1 << 27);
    }

    #[test]
    fn ep_parsing_is_validated_like_cpp() {
        let mut e = Engine::with_hash_mb(1);
        e.get_board("rnbqkbnr/pppp1ppp/8/8/3pP3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        assert_eq!(e.pos.cur().ep_square, 20);
        e.get_board("rnbqkbnr/pppppppp/8/8/8/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        assert_eq!(e.pos.cur().ep_square, 0);
    }
}
