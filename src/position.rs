//! Board representation, position state, FEN parsing and make/unmake.
//!
//! The layout mirrors `GBoard`, `GData` and the global `Board[]`/`Data[]`
//! arrays.  `Position::data` is the search stack; `current` is the C++
//! `Current - Data` index.  `make_move`/`unmake_move` are exact ports of the
//! templated C++ functions, including the incremental Zobrist keys, PST and
//! material bookkeeping.

use crate::search::{NullReporter, Reporter};
use crate::tables::{
    tables, Tables, PAWN_HASH_MASK, PAWN_HASH_SIZE, PRINCIPAL_VARIATION_MASK,
    PRINCIPAL_VARIATION_TABLE_SIZE,
};
use crate::types::*;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

pub const TRANSPOSITION_ENTRY_SIZE: usize = 16;

pub struct TranspositionTable {
    words: Vec<std::sync::atomic::AtomicU64>,
    entries: usize,
}

impl Default for TranspositionTable {
    fn default() -> Self {
        TranspositionTable {
            words: Vec::new(),
            entries: 0,
        }
    }
}

impl TranspositionTable {
    pub fn new(entries: usize) -> Self {
        TranspositionTable {
            words: (0..entries * 2)
                .map(|_| std::sync::atomic::AtomicU64::new(0))
                .collect(),
            entries,
        }
    }

    #[inline(always)]
    pub fn entries(&self) -> usize {
        self.entries
    }

    #[inline(always)]
    pub fn load(&self, i: usize) -> TranspositionEntry {
        use std::sync::atomic::Ordering;
        debug_assert!(i < self.entries);
        let w0 = unsafe { self.words.get_unchecked(i * 2) }.load(Ordering::Relaxed);
        let w1 = unsafe { self.words.get_unchecked(i * 2 + 1) }.load(Ordering::Relaxed);
        TranspositionEntry {
            key: w0 as u32,
            date: (w0 >> 32) as u16,
            chess_move: (w0 >> 48) as u16,
            low: w1 as u16 as i16,
            high: (w1 >> 16) as u16 as i16,
            flags: (w1 >> 32) as u16,
            low_depth: (w1 >> 48) as u8,
            high_depth: (w1 >> 56) as u8,
        }
    }

    #[inline(always)]
    pub fn store(&self, i: usize, e: TranspositionEntry) {
        use std::sync::atomic::Ordering;
        let w0 = e.key as u64 | ((e.date as u64) << 32) | ((e.chess_move as u64) << 48);
        let w1 = (e.low as u16 as u64)
            | ((e.high as u16 as u64) << 16)
            | ((e.flags as u64) << 32)
            | ((e.low_depth as u64) << 48)
            | ((e.high_depth as u64) << 56);
        debug_assert!(i < self.entries);
        unsafe { self.words.get_unchecked(i * 2) }.store(w0, Ordering::Relaxed);
        unsafe { self.words.get_unchecked(i * 2 + 1) }.store(w1, Ordering::Relaxed);
    }

    pub fn clear(&self) {
        use std::sync::atomic::Ordering;
        for w in &self.words {
            w.store(0, Ordering::Relaxed);
        }
    }

    pub fn reset_dates(&self, date: u16) {
        for i in 0..self.entries {
            let mut e = self.load(i);
            e.date = date;
            self.store(i, e);
        }
    }
}

pub struct PrincipalVariationTable {
    words: Vec<std::sync::atomic::AtomicU64>,
    entries: usize,
}

impl Default for PrincipalVariationTable {
    fn default() -> Self {
        PrincipalVariationTable {
            words: Vec::new(),
            entries: 0,
        }
    }
}

impl PrincipalVariationTable {
    pub fn new(entries: usize) -> Self {
        PrincipalVariationTable {
            words: (0..entries * 3)
                .map(|_| std::sync::atomic::AtomicU64::new(0))
                .collect(),
            entries,
        }
    }

    #[inline(always)]
    pub fn entries(&self) -> usize {
        self.entries
    }

    #[inline(always)]
    pub fn load(&self, i: usize) -> PrincipalVariationEntry {
        use std::sync::atomic::Ordering;
        debug_assert!(i < self.entries);
        let w0 = unsafe { self.words.get_unchecked(i * 3) }.load(Ordering::Relaxed);
        let w1 = unsafe { self.words.get_unchecked(i * 3 + 1) }.load(Ordering::Relaxed);
        let w2 = unsafe { self.words.get_unchecked(i * 3 + 2) }.load(Ordering::Relaxed);
        PrincipalVariationEntry {
            key: w0 as u32,
            date: (w0 >> 32) as u16,
            chess_move: (w0 >> 48) as u16,
            value: w1 as u16 as i16,
            exclusion: (w1 >> 16) as u16 as i16,
            depth: (w1 >> 32) as u8,
            ex_depth: (w1 >> 40) as u8,
            knodes: w2 as u32 as i32,
            ply: (w2 >> 32) as u32 as i32,
        }
    }

    #[inline(always)]
    pub fn store(&self, i: usize, e: PrincipalVariationEntry) {
        use std::sync::atomic::Ordering;
        let w0 = e.key as u64 | ((e.date as u64) << 32) | ((e.chess_move as u64) << 48);
        let w1 = (e.value as u16 as u64)
            | ((e.exclusion as u16 as u64) << 16)
            | ((e.depth as u64) << 32)
            | ((e.ex_depth as u64) << 40);
        let w2 = (e.knodes as u32 as u64) | ((e.ply as u32 as u64) << 32);
        debug_assert!(i < self.entries);
        unsafe { self.words.get_unchecked(i * 3) }.store(w0, Ordering::Relaxed);
        unsafe { self.words.get_unchecked(i * 3 + 1) }.store(w1, Ordering::Relaxed);
        unsafe { self.words.get_unchecked(i * 3 + 2) }.store(w2, Ordering::Relaxed);
    }

    pub fn clear(&self) {
        use std::sync::atomic::Ordering;
        for w in &self.words {
            w.store(0, Ordering::Relaxed);
        }
    }

    pub fn reset_dates(&self, date: u16) {
        for i in 0..self.entries {
            let mut e = self.load(i);
            e.date = date;
            self.store(i, e);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TranspositionEntry {
    pub key: u32,
    pub date: u16,
    pub chess_move: u16,
    pub low: i16,
    pub high: i16,
    pub flags: u16,
    pub low_depth: u8,
    pub high_depth: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrincipalVariationEntry {
    pub key: u32,
    pub date: u16,
    pub chess_move: u16,
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
    pub passed_pawns: [u8; 2],
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
    pub refutation_moves: [u16; 2],
    pub check_refs: [u16; 2],
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub num_threads: usize,
    pub multi_principal_variation: usize,
    pub syzygy_probe_depth: usize,
    pub transposition_size: usize,
    pub parent_process_id: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            num_threads: 1,
            multi_principal_variation: 1,
            syzygy_probe_depth: 1,
            transposition_size: 128 * (1 << 20),
            parent_process_id: 0,
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
    pub bitboards: Bitboards,
    pub squares: Squares,
}

impl Default for Board {
    fn default() -> Self {
        Board {
            bitboards: Bitboards::default(),
            squares: Squares::default(),
        }
    }
}

impl Board {
    #[inline(always)]
    pub fn clear(&mut self) {
        self.bitboards = Bitboards::default();
        self.squares = Squares::default();
    }

    #[inline(always)]
    pub fn set(&mut self, s: Square, piece: Piece) {
        self.squares[s] = piece as u8;
        if piece != 0 {
            self.bitboards[piece] |= bit(s);
            self.bitboards[piece & 1] |= bit(s);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    pub key: u64,
    pub pawn_key: u64,
    pub eval_key: u64,
    pub attacks: [u64; 2],
    pub pawn_attacks: [u64; 2],
    pub passed_pawns: u64,
    pub xray_attacks: [u64; 2],
    pub pinned_pieces: [u64; 2],
    pub threats: u64,
    pub move_mask: u64,
    pub side_to_move: u8,
    pub castling_rights: u8,
    pub ply: u8,
    pub en_passant_square: u8,
    pub captured_piece: u8,
    pub generation_flags: u8,
    pub moving_piece: u8,
    pub stage: u8,
    pub evaluation_multiplier: u8,
    pub score: i16,
    pub chess_move: u16,
    pub killer_moves: [u16; 3],
    pub refutation_moves: [u16; 2],
    pub best_move: i32,
    pub material: i32,
    pub piece_square_score: i32,
    pub margin: i32,
    pub start_index: usize,
    pub current: usize,
}

impl Default for State {
    fn default() -> Self {
        State {
            key: 0,
            pawn_key: 0,
            eval_key: 0,
            attacks: [0; 2],
            pawn_attacks: [0; 2],
            passed_pawns: 0,
            xray_attacks: [0; 2],
            pinned_pieces: [0; 2],
            threats: 0,
            move_mask: 0,
            side_to_move: 0,
            castling_rights: 0,
            ply: 0,
            en_passant_square: 0,
            captured_piece: 0,
            generation_flags: 0,
            moving_piece: 0,
            stage: 0,
            evaluation_multiplier: 0,
            score: 0,
            chess_move: 0,
            killer_moves: [0; 3],
            refutation_moves: [0; 2],
            best_move: 0,
            material: 0,
            piece_square_score: 0,
            margin: 0,
            start_index: 0,
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
pub struct StateStack([State; MAX_PLY]);

impl Default for StateStack {
    fn default() -> Self {
        StateStack([State::default(); MAX_PLY])
    }
}

impl std::ops::Index<usize> for StateStack {
    type Output = State;
    #[inline(always)]
    fn index(&self, i: usize) -> &State {
        debug_assert!(i < MAX_PLY);
        unsafe { self.0.get_unchecked(i) }
    }
}

impl std::ops::IndexMut<usize> for StateStack {
    #[inline(always)]
    fn index_mut(&mut self, i: usize) -> &mut State {
        debug_assert!(i < MAX_PLY);
        unsafe { self.0.get_unchecked_mut(i) }
    }
}

impl std::ops::Deref for StateStack {
    type Target = [State];
    #[inline(always)]
    fn deref(&self) -> &[State] {
        &self.0
    }
}

impl std::ops::DerefMut for StateStack {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [State] {
        &mut self.0
    }
}

#[derive(Clone)]
pub struct Position {
    pub board: Board,
    pub states: StateStack,
    pub current: usize,
    pub stack: [u64; STACK_SIZE],
    pub stack_pointer: usize,
    pub nodes: u64,
    pub tablebase_hits: u64,
    pub root_depth: u32,
}

impl Default for Position {
    fn default() -> Self {
        Position {
            board: Board::default(),
            states: StateStack::default(),
            current: 0,
            stack: [0; STACK_SIZE],
            stack_pointer: 0,
            nodes: 0,
            tablebase_hits: 0,
            root_depth: 1,
        }
    }
}

impl Position {
    #[inline(always)]
    pub fn current(&self) -> &State {
        &self.states[self.current]
    }

    #[inline(always)]
    pub fn cur_mut(&mut self) -> &mut State {
        &mut self.states[self.current]
    }

    #[inline(always)]
    pub fn side_to_move(&self) -> Color {
        self.states[self.current].side_to_move as Color
    }
}

/// The complete engine state: immutable tables, position stack, hash tables
/// and search bookkeeping.
pub struct Engine {
    pub tables: &'static Tables,
    pub pos: Position,
    pub pawn_hash: Vec<PawnEntry>,
    pub pawn_hash_mask: usize,
    pub transposition_table: Arc<TranspositionTable>,
    pub principal_variation_table: Arc<PrincipalVariationTable>,
    pub tablebases: Option<Arc<crate::tablebase::GullTableBases>>,
    pub tablebase_max: usize,
    pub transposition_mask: usize,
    pub principal_variation_mask: usize,
    pub history: [i16; 16 * 64],
    pub delta: Box<[i16; 16 * 4096]>,
    pub reference: Box<[Reference; 16 * 64]>,
    pub root_list: [i32; 256],
    pub move_lists: Box<MoveLists>,
    pub multi_principal_variation: [i32; 256],
    pub settings: Settings,
    pub shared: crate::search::SharedInfo,
    pub stats: crate::search::SearchStats,
    pub search_info: crate::search::SearchInfo,
    pub base_search_info: crate::search::SearchInfo,
    pub last_depth: i32,
    pub last_time: i64,
    pub last_value: i32,
    pub last_exact_value: i32,
    pub last_speed: i64,
    pub prev_move: i32,
    pub inst_cnt: i32,
    pub principal_variation_hashing: i32,
    pub search_moves: i32,
    pub previous: i32,
    pub aspiration: i32,
    pub info_time: i64,
    pub thread_id: u32,
    pub shared_nodes: Arc<AtomicU64>,
    pub nodes_base: u64,
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
        let bytes = transposition_size_bytes(mb as u64);
        let entries = bytes / TRANSPOSITION_ENTRY_SIZE;
        debug_assert!(entries.is_power_of_two());
        let mut e = Engine {
            tables: tables(),
            pos: Position::default(),
            pawn_hash: vec![PawnEntry::default(); PAWN_HASH_SIZE],
            pawn_hash_mask: PAWN_HASH_MASK,
            transposition_table: Arc::new(TranspositionTable::default()),
            principal_variation_table: Arc::new(PrincipalVariationTable::default()),
            tablebases: None,
            tablebase_max: 0,
            transposition_mask: entries - 4,
            principal_variation_mask: PRINCIPAL_VARIATION_MASK,
            history: [0; 16 * 64],
            delta: Box::new([0; 16 * 4096]),
            reference: Box::new([Reference::default(); 16 * 64]),
            root_list: [0; 256],
            move_lists: Box::new(MoveLists::default()),
            multi_principal_variation: [0; 256],
            settings: Settings {
                transposition_size: bytes,
                ..Settings::default()
            },
            shared: crate::search::SharedInfo::default(),
            stats: crate::search::SearchStats::default(),
            search_info: crate::search::SearchInfo::default(),
            base_search_info: crate::search::SearchInfo::default(),
            last_depth: 128,
            last_time: 0,
            last_value: 0,
            last_exact_value: 0,
            last_speed: 0,
            prev_move: 0,
            inst_cnt: 0,
            principal_variation_hashing: 1,
            search_moves: 0,
            previous: 0,
            aspiration: 1,
            info_time: 0,
            thread_id: 0,
            shared_nodes: Arc::new(AtomicU64::new(0)),
            nodes_base: 0,
            reporter: Box::new(NullReporter),
            control: crate::search::SearchControl::default(),
            save_board: Board::default(),
            save_state: State::default(),
            save_sp: 0,
        };
        e.initialize_search(true);
        e
    }

    /// Create a Lazy-SMP worker sharing the transposition/PV tables with this
    /// engine.  History, refutation, pawn hash and position state are private.
    pub fn fork_for_thread(&self, thread_id: u32, pawn_entries: usize) -> Engine {
        let pawn_entries = pawn_entries.max(1);
        let mut settings = self.settings;
        settings.num_threads = 1;
        Engine {
            tables: self.tables,
            pos: self.pos.clone(),
            pawn_hash: vec![PawnEntry::default(); pawn_entries],
            pawn_hash_mask: pawn_entries - 1,
            transposition_table: Arc::clone(&self.transposition_table),
            principal_variation_table: Arc::clone(&self.principal_variation_table),
            tablebases: self.tablebases.clone(),
            tablebase_max: self.tablebase_max,
            transposition_mask: self.transposition_mask,
            principal_variation_mask: self.principal_variation_mask,
            history: self.history,
            delta: self.delta.clone(),
            reference: self.reference.clone(),
            root_list: self.root_list,
            move_lists: Box::new(MoveLists::default()),
            multi_principal_variation: self.multi_principal_variation,
            settings,
            shared: self.shared,
            stats: crate::search::SearchStats::default(),
            search_info: crate::search::SearchInfo::default(),
            base_search_info: crate::search::SearchInfo::default(),
            last_depth: 128,
            last_time: 0,
            last_value: 0,
            last_exact_value: 0,
            last_speed: 0,
            prev_move: 0,
            inst_cnt: 0,
            principal_variation_hashing: self.principal_variation_hashing,
            search_moves: self.search_moves,
            previous: 0,
            aspiration: 1,
            info_time: 0,
            thread_id,
            shared_nodes: Arc::clone(&self.shared_nodes),
            nodes_base: 0,
            reporter: Box::new(NullReporter),
            control: self.control.clone(),
            save_board: Board::default(),
            save_state: State::default(),
            save_sp: 0,
        }
    }

    /// Load a Syzygy path (UCI `SyzygyPath`).  An empty path disables
    /// tablebases.  Returns the number of supported pieces, or 0 when disabled.
    pub fn set_syzygy_path(&mut self, path: &str) -> Result<usize, String> {
        // Drop the old handle first: pyrrhic-rs uses a process-wide singleton.
        self.tablebases = None;
        self.tablebase_max = 0;
        match crate::tablebase::load(path) {
            Ok(Some((tb, pieces))) => {
                self.tablebases = Some(tb);
                self.tablebase_max = pieces;
                Ok(pieces)
            }
            Ok(None) => Ok(0),
            Err(e) => Err(e),
        }
    }

    #[inline]
    pub fn tablebase_pieces(&self) -> usize {
        self.tablebase_max
    }

    /// Set the Lazy SMP thread count (UCI `Threads`), clamped to 1..=256.
    pub fn set_threads(&mut self, threads: usize) {
        self.settings.num_threads = threads.clamp(1, 256);
    }

    /// Resize the transposition table; `mb` follows the UCI Hash option.
    pub fn resize_hash(&mut self, mb: u64) {
        let bytes = transposition_size_bytes(mb);
        self.settings.transposition_size = bytes;
        self.initialize_search(true);
    }

    pub fn initialize_search(&mut self, clear_hash: bool) {
        self.history.fill(0x0101);
        self.delta.fill(0);
        self.reference.fill(Reference::default());
        for s in self.pos.states.iter_mut().skip(1).take(127) {
            *s = State::default();
        }
        if clear_hash {
            self.shared.date = 1;
            self.transposition_table.clear();
            self.principal_variation_table.clear();
        }
        let bytes = self.settings.transposition_size;
        let entries = bytes / TRANSPOSITION_ENTRY_SIZE;
        debug_assert!(entries.is_power_of_two());
        if self.transposition_table.entries() != entries {
            self.transposition_table = Arc::new(TranspositionTable::new(entries));
        }
        self.transposition_mask = entries - 4;
        self.principal_variation_mask = PRINCIPAL_VARIATION_MASK;
        if self.principal_variation_table.entries() != PRINCIPAL_VARIATION_TABLE_SIZE {
            self.principal_variation_table =
                Arc::new(PrincipalVariationTable::new(PRINCIPAL_VARIATION_TABLE_SIZE));
        }
        self.set_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        self.last_time = 0;
        self.last_value = 0;
        self.last_exact_value = 0;
        self.inst_cnt = 0;
        self.last_speed = 0;
        self.search_moves = 0;
        self.last_depth = 128;
        self.search_info = crate::search::SearchInfo::default();
        self.base_search_info = crate::search::SearchInfo::default();
        self.previous = 0;
        self.prev_move = 0;
        self.info_time = 0;
        self.pos.nodes = 0;
        self.pos.tablebase_hits = 0;
    }

    pub fn setup_board(&mut self) {
        let tables = &self.tables;
        self.pos.stack_pointer = 0;
        self.shared.date = self.shared.date.wrapping_add(1);
        if self.shared.date > 0x8000 {
            self.shared.date = 2;
            self.transposition_table.reset_dates(1);
            self.principal_variation_table.reset_dates(1);
        }
        {
            let state = &mut self.pos.states[0];
            state.material = 0;
            state.piece_square_score = 0;
            state.key = tables.piece_keys[0][0];
            if state.side_to_move != 0 {
                state.key ^= tables.side_to_move_key;
            }
            state.key ^= tables.castling_keys[state.castling_rights as usize];
            if state.en_passant_square != 0 {
                state.key ^= tables.en_passant_keys[file(state.en_passant_square as usize)];
            }
            state.pawn_key = 0;
            state.pawn_key ^= tables.castling_keys[state.castling_rights as usize];
        }
        for i in 0..16 {
            self.pos.board.bitboards[i] = 0;
        }
        let mut occupancy = 0u64;
        for i in 0..64 {
            let piece = self.pos.board.squares[i] as usize;
            if piece != 0 {
                self.pos.board.bitboards[piece] |= bit(i);
                self.pos.board.bitboards[piece & 1] |= bit(i);
                occupancy |= bit(i);
                let state = &mut self.pos.states[0];
                state.key ^= tables.piece_keys[piece][i];
                if piece < WHITE_KNIGHT {
                    state.pawn_key ^= tables.piece_keys[piece][i];
                }
                if piece < WHITE_KING {
                    state.material += MATERIAL_CODE[piece];
                } else {
                    state.pawn_key ^= tables.piece_keys[piece][i];
                }
                state.piece_square_score += tables.piece_square_score[(piece << 6) | i];
            }
        }
        let _ = occupancy;
        let state = &mut self.pos.states[0];
        if population_count(self.pos.board.bitboards[WHITE_KNIGHT]) > 2
            || population_count(self.pos.board.bitboards[WHITE_LIGHT_BISHOP]) > 1
            || population_count(self.pos.board.bitboards[WHITE_DARK_BISHOP]) > 1
            || population_count(self.pos.board.bitboards[WHITE_ROOK]) > 2
            || population_count(self.pos.board.bitboards[WHITE_QUEEN]) > 2
        {
            state.material |= FLAG_UNUSUAL_MATERIAL;
        }
        if population_count(self.pos.board.bitboards[BLACK_KNIGHT]) > 2
            || population_count(self.pos.board.bitboards[BLACK_LIGHT_BISHOP]) > 1
            || population_count(self.pos.board.bitboards[BLACK_DARK_BISHOP]) > 1
            || population_count(self.pos.board.bitboards[BLACK_ROOK]) > 2
            || population_count(self.pos.board.bitboards[BLACK_QUEEN]) > 2
        {
            state.material |= FLAG_UNUSUAL_MATERIAL;
        }
        state.captured_piece = 0;
        state.killer_moves[1] = 0;
        state.killer_moves[2] = 0;
        state.ply = 0;
        self.pos.current = 0;
        self.pos.stack_pointer = 0;
        self.pos.stack[0] = state.key;
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
        let color = self.pos.current().side_to_move as Color;
        let piece = self.pos.board.squares[from] as usize;
        let capture = self.pos.board.squares[to] as usize;
        // Basic sanity checks.  The C++ front end trusts the GUI and applies
        // the move directly; we do the same, but reject obviously broken input
        // instead of corrupting the board.
        if piece == 0 || (piece & 1) != color as usize {
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
        let mut chess_move: i32 = ((from as i32) << 6) | (to as i32);
        if piece >= WHITE_KING && (to as i32 - from as i32).abs() == 2 {
            chess_move |= FLAG_CASTLING;
        }
        if self.pos.states[self.pos.current].en_passant_square != 0
            && to == self.pos.states[self.pos.current].en_passant_square as usize
            && piece == pawn_piece(color)
        {
            chess_move |= FLAG_EN_PASSANT;
        }
        if text.len() >= 5 {
            chess_move |= match text.as_bytes()[4].to_ascii_lowercase() {
                b'q' => FLAG_PROMOTION_QUEEN,
                b'r' => FLAG_PROMOTION_ROOK,
                b'n' => FLAG_PROMOTION_KNIGHT,
                b'b' => {
                    if ((to ^ (to >> 3)) & 1) != 0 {
                        FLAG_PROMOTION_LIGHT_BISHOP
                    } else {
                        FLAG_PROMOTION_DARK_BISHOP
                    }
                }
                _ => 0,
            };
        }

        self.make_move(color, chess_move);
        // UCI positions are always rooted at Data[0].  The C++ front end
        // copies the resulting position back to Data[0] after every move and
        // truncates the repetition stack to the current halfmove clock.
        let current = self.pos.current;
        self.pos.states[0] = self.pos.states[current];
        self.pos.current = 0;
        let ply = self.pos.states[0].ply as usize;
        if self.pos.stack_pointer >= ply {
            self.pos
                .stack
                .copy_within(self.pos.stack_pointer - ply..=self.pos.stack_pointer, 0);
            self.pos.stack_pointer = ply;
        }
        self.shared.root_depth = self.shared.root_depth.wrapping_add(1);
        true
    }

    pub fn set_fen(&mut self, fen: &str) -> String {
        self.pos.current = 0;
        self.pos.board.clear();
        self.pos.states[0] = State::default();
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
                        let s = square_index(file as usize, rank as usize);
                        let piece = piece_from_char(c, s);
                        self.pos.board.squares[s] = piece as u8;
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

        self.pos.states[0].side_to_move = if side.starts_with('b') { 1 } else { 0 };
        let mut flags = 0u8;
        if castle.contains('K') {
            flags |= CASTLING_WHITE_KINGSIDE;
        }
        if castle.contains('Q') {
            flags |= CASTLING_WHITE_QUEENSIDE;
        }
        if castle.contains('k') {
            flags |= CASTLING_BLACK_KINGSIDE;
        }
        if castle.contains('q') {
            flags |= CASTLING_BLACK_QUEENSIDE;
        }
        self.pos.states[0].castling_rights = flags;

        let mut en_passant_square = 0u8;
        if ep.len() >= 2 && ep != "-" {
            if let Some(s) = parse_square(ep) {
                let side_to_move = self.pos.states[0].side_to_move;
                let j = s ^ 8;
                // Reproduce the C++ validation exactly.
                let valid = {
                    let empty = self.pos.board.squares[s] == 0;
                    let enemy = self.pos.board.squares[j] == (3 - side_to_move);
                    let adjacent = self.pos.board.squares[j - 1] == (side_to_move + 2)
                        || self.pos.board.squares[j + 1] == (side_to_move + 2);
                    empty && enemy && adjacent
                };
                if valid {
                    en_passant_square = s as u8;
                }
            }
        }
        self.pos.states[0].en_passant_square = en_passant_square;
        self.pos.states[0].ply = halfmove;
        self.shared.root_depth = fullmove;
        self.setup_board();
        fields.get(6..).map(|r| r.join(" ")).unwrap_or_default()
    }

    /// FEN of the current search state (including side to move, castling and
    /// en-passant square).  Used by search tracing.
    pub fn current_fen(&self) -> String {
        let state = &self.pos.states[self.pos.current];
        let mut out = String::new();
        for r in (0..8).rev() {
            let mut empty = 0;
            for f in 0..8 {
                let p = self.pos.board.squares[square_index(f, r)] as usize;
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
        out.push(if state.side_to_move == 0 { 'w' } else { 'b' });
        out.push(' ');
        let mut c = String::new();
        if state.castling_rights & CASTLING_WHITE_KINGSIDE != 0 {
            c.push('K');
        }
        if state.castling_rights & CASTLING_WHITE_QUEENSIDE != 0 {
            c.push('Q');
        }
        if state.castling_rights & CASTLING_BLACK_KINGSIDE != 0 {
            c.push('k');
        }
        if state.castling_rights & CASTLING_BLACK_QUEENSIDE != 0 {
            c.push('q');
        }
        if c.is_empty() {
            c.push('-');
        }
        out.push_str(&c);
        out.push(' ');
        if state.en_passant_square != 0 {
            out.push_str(&square_to_string(state.en_passant_square as usize));
        } else {
            out.push('-');
        }
        out.push_str(&format!(" {} 1", state.ply));
        out
    }

    /// Parse a FEN and return the board as a FEN placement string (used by
    /// tests and debugging).
    pub fn to_fen(&self) -> String {
        let mut out = String::new();
        for r in (0..8).rev() {
            let mut empty = 0;
            for f in 0..8 {
                let p = self.pos.board.squares[square_index(f, r)] as usize;
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
        let state = &self.pos.states[0];
        out.push(' ');
        out.push(if state.side_to_move == 0 { 'w' } else { 'b' });
        out.push(' ');
        let mut c = String::new();
        if state.castling_rights & CASTLING_WHITE_KINGSIDE != 0 {
            c.push('K');
        }
        if state.castling_rights & CASTLING_WHITE_QUEENSIDE != 0 {
            c.push('Q');
        }
        if state.castling_rights & CASTLING_BLACK_KINGSIDE != 0 {
            c.push('k');
        }
        if state.castling_rights & CASTLING_BLACK_QUEENSIDE != 0 {
            c.push('q');
        }
        if c.is_empty() {
            c.push('-');
        }
        out.push_str(&c);
        out.push(' ');
        if state.en_passant_square != 0 {
            out.push_str(&square_to_string(state.en_passant_square as usize));
        } else {
            out.push('-');
        }
        out.push_str(" 0 1");
        out
    }

    #[inline(always)]
    pub fn piece(&self, color: Color) -> u64 {
        self.pos.board.bitboards[color]
    }
    #[inline(always)]
    pub fn pawn(&self, color: Color) -> u64 {
        self.pos.board.bitboards[WHITE_PAWN | color]
    }
    #[inline(always)]
    pub fn knight(&self, color: Color) -> u64 {
        self.pos.board.bitboards[WHITE_KNIGHT | color]
    }
    #[inline(always)]
    pub fn bishop(&self, color: Color) -> u64 {
        self.pos.board.bitboards[WHITE_LIGHT_BISHOP | color]
            | self.pos.board.bitboards[WHITE_DARK_BISHOP | color]
    }
    #[inline(always)]
    pub fn rook(&self, color: Color) -> u64 {
        self.pos.board.bitboards[WHITE_ROOK | color]
    }
    #[inline(always)]
    pub fn queen(&self, color: Color) -> u64 {
        self.pos.board.bitboards[WHITE_QUEEN | color]
    }
    #[inline(always)]
    pub fn king(&self, color: Color) -> u64 {
        self.pos.board.bitboards[WHITE_KING | color]
    }
    #[inline(always)]
    pub fn non_pawn(&self, color: Color) -> u64 {
        self.piece(color) ^ self.pawn(color)
    }
    #[inline(always)]
    pub fn non_pawn_king(&self, color: Color) -> u64 {
        self.non_pawn(color) ^ self.king(color)
    }
    #[inline(always)]
    pub fn bslider(&self, color: Color) -> u64 {
        self.bishop(color) | self.queen(color)
    }
    #[inline(always)]
    pub fn rslider(&self, color: Color) -> u64 {
        self.rook(color) | self.queen(color)
    }
    #[inline(always)]
    pub fn major(&self, color: Color) -> u64 {
        self.rslider(color)
    }
    #[inline(always)]
    pub fn minor(&self, color: Color) -> u64 {
        self.knight(color) | self.bishop(color)
    }
    #[inline(always)]
    pub fn slider(&self, color: Color) -> u64 {
        self.bslider(color) | self.rslider(color)
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
    pub fn all_pawns(&self) -> u64 {
        self.pawn(WHITE) | self.pawn(BLACK)
    }
    #[inline(always)]
    pub fn non_pawn_king_all(&self) -> u64 {
        self.non_pawn_king(WHITE) | self.non_pawn_king(BLACK)
    }
    #[inline(always)]
    pub fn king_pos(&self, color: Color) -> Square {
        least_significant_bit(self.king(color))
    }
    #[inline(always)]
    pub fn piece_at(&self, s: Square) -> Piece {
        self.pos.board.squares[s] as Piece
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
    pub fn shift_west(color: Color, target: u64) -> u64 {
        if color == BLACK {
            Self::shift_sw(target)
        } else {
            Self::shift_nw(target)
        }
    }
    #[inline(always)]
    pub fn shift_east(color: Color, target: u64) -> u64 {
        if color == BLACK {
            Self::shift_se(target)
        } else {
            Self::shift_ne(target)
        }
    }
    #[inline(always)]
    pub fn shift(color: Color, target: u64) -> u64 {
        if color == BLACK {
            target >> 8
        } else {
            target << 8
        }
    }
    #[inline(always)]
    pub fn push(color: Color) -> i32 {
        if color == BLACK {
            -8
        } else {
            8
        }
    }
    #[inline(always)]
    pub fn push_west(color: Color) -> i32 {
        if color == BLACK {
            -9
        } else {
            7
        }
    }
    #[inline(always)]
    pub fn push_east(color: Color) -> i32 {
        if color == BLACK {
            -7
        } else {
            9
        }
    }
    #[inline(always)]
    pub fn dir(color: Color) -> i32 {
        if color == BLACK {
            -1
        } else {
            1
        }
    }
    #[inline(always)]
    pub fn is_greater(color: Color, x: Square, y: Square) -> bool {
        if color == BLACK {
            x < y
        } else {
            x > y
        }
    }

    #[inline(always)]
    pub fn line(&self, color: Color, n: usize) -> u64 {
        if color == BLACK {
            self.tables.line[7 - n]
        } else {
            self.tables.line[n]
        }
    }

    /// `make_move<color>`
    #[inline(always)]
    pub fn make_move(&mut self, color: Color, chess_move: Move) {
        if color == WHITE {
            self.do_move_c::<true>(chess_move);
        } else {
            self.do_move_c::<false>(chess_move);
        }
    }

    fn do_move_c<const ME: bool>(&mut self, chess_move: Move) {
        let color: Color = if ME { WHITE } else { BLACK };
        let tables = &self.tables;
        let cur_idx = self.pos.current;
        // Mutate the child state in place.  The C++ implementation writes into
        // the existing Data slot and relies on search to initialise transient
        // fields; copying the whole 168-byte parent state here is unnecessary
        // and is visible in the hot profile.
        let current = unsafe { &*(&self.pos.states[cur_idx] as *const State) };
        let next = unsafe { &mut *(&mut self.pos.states[cur_idx + 1] as *mut State) };
        next.en_passant_square = 0;
        let to = move_to(chess_move);
        let capture = self.pos.board.squares[to] as usize;
        let mut finish_via_capture = false;

        if capture == 0 {
            next.captured_piece = 0;
        } else {
            let from = move_from(chess_move);
            let mut piece = self.pos.board.squares[from] as usize;
            let opponent_color = opponent(color);
            next.side_to_move = opponent_color as u8;
            next.captured_piece = capture as u8;
            self.pos.board.squares[from] = 0;
            self.pos.board.squares[to] = piece as u8;
            next.moving_piece = piece as u8;
            let mask_from = bit(from);
            let mask_to = bit(to);
            self.pos.board.bitboards[piece] ^= mask_from;
            self.pos.board.bitboards[color] ^= mask_from;
            self.pos.board.bitboards[capture] ^= mask_to;
            self.pos.board.bitboards[opponent_color] ^= mask_to;
            self.pos.board.bitboards[piece] |= mask_to;
            self.pos.board.bitboards[color] |= mask_to;
            next.castling_rights = current.castling_rights
                & tables.castling_rights_update[to]
                & tables.castling_rights_update[from];
            next.piece_square_score = current.piece_square_score
                + tables.piece_square_score[(piece << 6) | to]
                - tables.piece_square_score[(piece << 6) | from]
                - tables.piece_square_score[(capture << 6) | to];
            next.key = current.key
                ^ tables.piece_keys[piece][from]
                ^ tables.piece_keys[piece][to]
                ^ tables.piece_keys[capture][to]
                ^ tables.castling_keys[current.castling_rights as usize]
                ^ tables.castling_keys[next.castling_rights as usize];
            if capture != pawn_piece(opponent_color) {
                next.pawn_key = current.pawn_key
                    ^ tables.castling_keys[current.castling_rights as usize]
                    ^ tables.castling_keys[next.castling_rights as usize];
            } else {
                next.pawn_key = current.pawn_key
                    ^ tables.piece_keys[pawn_piece(opponent_color)][to]
                    ^ tables.castling_keys[current.castling_rights as usize]
                    ^ tables.castling_keys[next.castling_rights as usize];
            }
            next.material = current.material - MATERIAL_CODE[capture];
            if (current.material & FLAG_UNUSUAL_MATERIAL) != 0 && capture >= WHITE_KNIGHT {
                if population_count(self.pos.board.bitboards[WHITE_QUEEN]) <= 2
                    && population_count(self.pos.board.bitboards[BLACK_QUEEN]) <= 2
                    && population_count(self.pos.board.bitboards[WHITE_LIGHT_BISHOP]) <= 1
                    && population_count(self.pos.board.bitboards[BLACK_LIGHT_BISHOP]) <= 1
                    && population_count(self.pos.board.bitboards[WHITE_KNIGHT]) <= 2
                    && population_count(self.pos.board.bitboards[BLACK_KNIGHT]) <= 2
                    && population_count(self.pos.board.bitboards[WHITE_ROOK]) <= 2
                    && population_count(self.pos.board.bitboards[BLACK_ROOK]) <= 2
                {
                    next.material ^= FLAG_UNUSUAL_MATERIAL;
                }
            }
            if piece == pawn_piece(color) {
                next.pawn_key ^=
                    tables.piece_keys[pawn_piece(color)][from] ^ tables.piece_keys[piece][to];
                if is_promotion(chess_move) {
                    piece = promotion_piece(chess_move, color);
                    self.pos.board.squares[to] = piece as u8;
                    next.material += MATERIAL_CODE[piece] - MATERIAL_CODE[pawn_piece(color)];
                    if piece < WHITE_ROOK {
                        if piece >= WHITE_LIGHT_BISHOP && self.pos.board.bitboards[piece] != 0 {
                            next.material |= FLAG_UNUSUAL_MATERIAL;
                        }
                        if has_multiple_bits(self.pos.board.bitboards[piece]) {
                            next.material |= FLAG_UNUSUAL_MATERIAL;
                        }
                    } else if has_multiple_bits(self.pos.board.bitboards[piece]) {
                        next.material |= FLAG_UNUSUAL_MATERIAL;
                    }
                    self.pos.board.bitboards[WHITE_PAWN | color] ^= mask_to;
                    self.pos.board.bitboards[piece] |= mask_to;
                    next.piece_square_score += tables.piece_square_score[(piece << 6) | to]
                        - tables.piece_square_score[(pawn_piece(color) << 6) | to];
                    next.key ^=
                        tables.piece_keys[piece][to] ^ tables.piece_keys[pawn_piece(color)][to];
                    next.pawn_key ^= tables.piece_keys[pawn_piece(color)][to];
                }
            } else if piece >= WHITE_KING {
                next.pawn_key ^= tables.piece_keys[piece][from] ^ tables.piece_keys[piece][to];
            }
            if current.en_passant_square != 0 {
                next.key ^= tables.en_passant_keys[file(current.en_passant_square as usize)];
            }
            next.side_to_move = (current.side_to_move ^ 1) as u8;
            next.key ^= tables.side_to_move_key;
            next.ply = 0;
            finish_via_capture = true;
        }

        if !finish_via_capture {
            let from = move_from(chess_move);
            next.ply = current.ply.wrapping_add(1);
            let mut piece = self.pos.board.squares[from] as usize;
            self.pos.board.squares[from] = 0;
            self.pos.board.squares[to] = piece as u8;
            next.moving_piece = piece as u8;
            let mask_from = bit(from);
            let mask_to = bit(to);
            self.pos.board.bitboards[piece] ^= mask_from;
            self.pos.board.bitboards[color] ^= mask_from;
            self.pos.board.bitboards[piece] |= mask_to;
            self.pos.board.bitboards[color] |= mask_to;
            next.castling_rights = current.castling_rights
                & tables.castling_rights_update[to]
                & tables.castling_rights_update[from];
            next.piece_square_score = current.piece_square_score
                + tables.piece_square_score[(piece << 6) | to]
                - tables.piece_square_score[(piece << 6) | from];
            next.key = current.key
                ^ tables.piece_keys[piece][to]
                ^ tables.piece_keys[piece][from]
                ^ tables.castling_keys[current.castling_rights as usize]
                ^ tables.castling_keys[next.castling_rights as usize];
            next.material = current.material;
            if piece == pawn_piece(color) {
                next.ply = 0;
                next.pawn_key = current.pawn_key
                    ^ tables.piece_keys[pawn_piece(color)][to]
                    ^ tables.piece_keys[pawn_piece(color)][from]
                    ^ tables.castling_keys[current.castling_rights as usize]
                    ^ tables.castling_keys[next.castling_rights as usize];
                if is_en_passant(chess_move) {
                    let cap_sq = to ^ 8;
                    self.pos.board.squares[cap_sq] = 0;
                    let u = bit(cap_sq);
                    next.key ^= tables.piece_keys[pawn_piece(opponent(color))][cap_sq];
                    next.pawn_key ^= tables.piece_keys[pawn_piece(opponent(color))][cap_sq];
                    next.piece_square_score -=
                        tables.piece_square_score[(pawn_piece(opponent(color)) << 6) | cap_sq];
                    self.pos.board.bitboards[WHITE_PAWN | opponent(color)] &= !u;
                    self.pos.board.bitboards[opponent(color)] &= !u;
                    next.material -= MATERIAL_CODE[pawn_piece(opponent(color))];
                } else if is_promotion(chess_move) {
                    piece = promotion_piece(chess_move, color);
                    self.pos.board.squares[to] = piece as u8;
                    next.material += MATERIAL_CODE[piece] - MATERIAL_CODE[pawn_piece(color)];
                    if piece < WHITE_ROOK {
                        if piece >= WHITE_LIGHT_BISHOP && self.pos.board.bitboards[piece] != 0 {
                            next.material |= FLAG_UNUSUAL_MATERIAL;
                        }
                        if has_multiple_bits(self.pos.board.bitboards[piece]) {
                            next.material |= FLAG_UNUSUAL_MATERIAL;
                        }
                    } else if has_multiple_bits(self.pos.board.bitboards[piece]) {
                        next.material |= FLAG_UNUSUAL_MATERIAL;
                    }
                    self.pos.board.bitboards[WHITE_PAWN | color] ^= mask_to;
                    self.pos.board.bitboards[piece] |= mask_to;
                    next.piece_square_score += tables.piece_square_score[(piece << 6) | to]
                        - tables.piece_square_score[(pawn_piece(color) << 6) | to];
                    next.key ^=
                        tables.piece_keys[piece][to] ^ tables.piece_keys[pawn_piece(color)][to];
                    next.pawn_key ^= tables.piece_keys[pawn_piece(color)][to];
                } else if (to ^ from) == 16 {
                    if tables.pawn_attacks[color][(to + from) >> 1] & self.pawn(opponent(color))
                        != 0
                    {
                        next.en_passant_square = ((to + from) >> 1) as u8;
                        next.key ^= tables.en_passant_keys[file(next.en_passant_square as usize)];
                    }
                }
            } else {
                if piece < WHITE_KING {
                    next.pawn_key = current.pawn_key
                        ^ tables.castling_keys[current.castling_rights as usize]
                        ^ tables.castling_keys[next.castling_rights as usize];
                } else {
                    next.pawn_key = current.pawn_key
                        ^ tables.piece_keys[piece][to]
                        ^ tables.piece_keys[piece][from]
                        ^ tables.castling_keys[current.castling_rights as usize]
                        ^ tables.castling_keys[next.castling_rights as usize];
                }
                if is_castling(chess_move) {
                    let (rold, rnew) = match to {
                        6 => (7, 5),
                        2 => (0, 3),
                        62 => (63, 61),
                        58 => (56, 59),
                        _ => (0, 0),
                    };
                    self.pos.board.squares[rold] = 0;
                    self.pos.board.squares[rnew] = rook_piece(color) as u8;
                    self.pos.board.bitboards[rook_piece(color)] ^= bit(rold);
                    self.pos.board.bitboards[color] ^= bit(rold);
                    self.pos.board.bitboards[rook_piece(color)] |= bit(rnew);
                    self.pos.board.bitboards[color] |= bit(rnew);
                    next.piece_square_score += tables.piece_square_score
                        [(rook_piece(color) << 6) | rnew]
                        - tables.piece_square_score[(rook_piece(color) << 6) | rold];
                    next.key ^= tables.piece_keys[rook_piece(color)][rnew]
                        ^ tables.piece_keys[rook_piece(color)][rold];
                }
            }
            if current.en_passant_square != 0 {
                next.key ^= tables.en_passant_keys[file(current.en_passant_square as usize)];
            }
            next.side_to_move = opponent(color) as u8;
            next.key ^= tables.side_to_move_key;
        }

        self.pos.stack_pointer += 1;
        self.pos.stack[self.pos.stack_pointer] = next.key;
        next.chess_move = chess_move as u16;
        next.generation_flags = 0;
        self.pos.current += 1;
        self.pos.nodes += 1;
    }

    /// `unmake_move<color>`
    #[inline(always)]
    pub fn unmake_move(&mut self, color: Color, chess_move: Move) {
        if color == WHITE {
            self.undo_move_c::<true>(chess_move);
        } else {
            self.undo_move_c::<false>(chess_move);
        }
    }

    fn undo_move_c<const ME: bool>(&mut self, chess_move: Move) {
        let color: Color = if ME { WHITE } else { BLACK };
        let to = move_to(chess_move);
        let from = move_from(chess_move);
        let capture = self.pos.states[self.pos.current].captured_piece;
        if is_promotion(chess_move) {
            let p = self.pos.board.squares[to] as usize;
            self.pos.board.bitboards[p] ^= bit(to);
            let piece = pawn_piece(color);
            self.pos.board.squares[from] = piece as u8;
            self.pos.board.bitboards[piece] |= bit(from);
            self.pos.board.bitboards[color] |= bit(from);
            self.pos.board.bitboards[piece] &= !bit(to);
            self.pos.board.bitboards[color] ^= bit(to);
        } else {
            let piece = self.pos.board.squares[to] as usize;
            self.pos.board.squares[from] = piece as u8;
            self.pos.board.bitboards[piece] |= bit(from);
            self.pos.board.bitboards[color] |= bit(from);
            self.pos.board.bitboards[piece] &= !bit(to);
            self.pos.board.bitboards[color] ^= bit(to);
        }
        self.pos.board.squares[to] = capture;
        if capture != 0 {
            let c = capture as usize;
            self.pos.board.bitboards[c] |= bit(to);
            self.pos.board.bitboards[opponent(color)] |= bit(to);
        } else if is_castling(chess_move) {
            let (rold, rnew) = match to {
                6 => (7, 5),
                2 => (0, 3),
                62 => (63, 61),
                58 => (56, 59),
                _ => (0, 0),
            };
            self.pos.board.squares[rnew] = 0;
            self.pos.board.squares[rold] = rook_piece(color) as u8;
            self.pos.board.bitboards[WHITE_ROOK | color] ^= bit(rnew);
            self.pos.board.bitboards[color] ^= bit(rnew);
            self.pos.board.bitboards[WHITE_ROOK | color] |= bit(rold);
            self.pos.board.bitboards[color] |= bit(rold);
        } else if is_en_passant(chess_move) {
            let square_index = to ^ 8;
            let piece = pawn_piece(opponent(color));
            self.pos.board.squares[square_index] = piece as u8;
            self.pos.board.bitboards[opponent(color)] |= bit(square_index);
            self.pos.board.bitboards[WHITE_PAWN | opponent(color)] |= bit(square_index);
        }
        self.pos.current -= 1;
        self.pos.stack_pointer -= 1;
    }

    /// `make_null_move()`
    #[inline(always)]
    pub fn make_null_move(&mut self) {
        let tables = &self.tables;
        let cur_idx = self.pos.current;
        let current = unsafe { &*(&self.pos.states[cur_idx] as *const State) };
        let next = unsafe { &mut *(&mut self.pos.states[cur_idx + 1] as *mut State) };
        next.key = current.key ^ tables.side_to_move_key;
        next.pawn_key = current.pawn_key;
        next.eval_key = 0;
        next.side_to_move = current.side_to_move ^ 1;
        next.material = current.material;
        next.piece_square_score = current.piece_square_score;
        next.ply = 0;
        next.castling_rights = current.castling_rights;
        next.en_passant_square = 0;
        next.captured_piece = 0;
        if current.en_passant_square != 0 {
            next.key ^= tables.en_passant_keys[file(current.en_passant_square as usize)];
        }
        next.attacks[WHITE] = current.attacks[WHITE];
        next.attacks[BLACK] = current.attacks[BLACK];
        next.pawn_attacks[WHITE] = current.pawn_attacks[WHITE];
        next.pawn_attacks[BLACK] = current.pawn_attacks[BLACK];
        next.xray_attacks[WHITE] = current.xray_attacks[WHITE];
        next.xray_attacks[BLACK] = current.xray_attacks[BLACK];
        next.pinned_pieces[WHITE] = current.pinned_pieces[WHITE];
        next.pinned_pieces[BLACK] = current.pinned_pieces[BLACK];
        next.threats = current.threats;
        next.passed_pawns = current.passed_pawns;
        next.score = current.score.wrapping_neg();
        next.chess_move = 0;
        next.generation_flags = 0;
        self.pos.stack_pointer += 1;
        self.pos.stack[self.pos.stack_pointer] = next.key;
        self.pos.current += 1;
        self.pos.nodes += 1;
    }

    pub fn unmake_null_move(&mut self) {
        self.pos.current -= 1;
        self.pos.stack_pointer -= 1;
    }

    // -- root snapshots ----------------------------------------------------

    pub fn save_root(&mut self) {
        self.save_board = self.pos.board;
        self.save_state = self.pos.states[0];
        self.save_sp = self.pos.stack_pointer;
        self.shared.root_board = self.pos.board;
        self.shared.root_state = self.pos.states[0];
        self.shared.root_sp = self.pos.stack_pointer;
        self.shared.root_stack = self.pos.stack;
    }

    pub fn restore_root(&mut self) {
        self.pos.board = self.save_board;
        self.pos.states[0] = self.save_state;
        self.pos.current = 0;
        self.pos.stack_pointer = self.save_sp;
    }

    pub fn restore_shared_root(&mut self) {
        self.pos.board = self.shared.root_board;
        self.pos.states[0] = self.shared.root_state;
        self.pos.current = 0;
        self.pos.stack_pointer = self.shared.root_sp;
        self.pos.stack = self.shared.root_stack;
    }
}

/// `FILE_H` and `LINE_7` helpers used by the shift macros.
pub const FILE_H: u64 = 0x8080_8080_8080_8080;
pub const LINE_7: u64 = 0xFF00_0000_0000_0000;

/// `Hash` option conversion: `HASH_SIZE(n)` from the C++ source.
pub fn transposition_size_bytes(n: u64) -> usize {
    if n == 0 {
        1 << 20
    } else {
        let bytes = (n as usize).saturating_mul(1 << 20);
        let most_significant_bit = (usize::BITS - 1 - bytes.leading_zeros()) as usize;
        1usize << most_significant_bit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startpos_keys_match_reference() {
        let e = Engine::with_hash_mb(1);
        // Values produced by the C++ reference harness for the start position.
        assert_eq!(e.pos.current().pawn_key, 0x78746e165ee92e55);
        assert_eq!(e.pos.current().key, 0x21d09a40dda19091);
        assert_eq!(e.pos.current().material, 944779);
        assert_eq!(e.pos.current().piece_square_score, 0);
        assert_eq!(e.pos.current().side_to_move, 0);
    }

    #[test]
    fn kiwipete_keys_match_reference() {
        let mut e = Engine::with_hash_mb(1);
        e.set_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
        assert_eq!(e.pos.current().pawn_key, 0xf7904d3200f0397e);
        assert_eq!(e.pos.current().key, 0x70537df50de78123);
        assert_eq!(e.pos.current().material, 944779);
        assert_eq!(
            e.to_fen(),
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"
        );
    }

    #[test]
    fn fen_roundtrip_bishops_and_castling() {
        let mut e = Engine::with_hash_mb(1);
        let fen = "r1bqk2r/pppp1ppp/2n2n2/2b1p3/2B1P3/2N2N2/PPPP1PPP/R1BQK2R w KQkq - 0 1";
        e.set_fen(fen);
        assert_eq!(e.to_fen(), fen);
        assert_eq!(
            e.pos.board.bitboards[WHITE_LIGHT_BISHOP].count_ones()
                + e.pos.board.bitboards[WHITE_DARK_BISHOP].count_ones(),
            2
        );
        assert_eq!(
            e.pos.board.bitboards[BLACK_LIGHT_BISHOP].count_ones()
                + e.pos.board.bitboards[BLACK_DARK_BISHOP].count_ones(),
            2
        );
    }

    #[test]
    fn hash_size_formula() {
        assert_eq!(transposition_size_bytes(0), 1 << 20);
        assert_eq!(transposition_size_bytes(1), 1 << 20);
        assert_eq!(transposition_size_bytes(2), 1 << 21);
        assert_eq!(transposition_size_bytes(3), 1 << 21);
        assert_eq!(transposition_size_bytes(8), 1 << 23);
        assert_eq!(transposition_size_bytes(128), 1 << 27);
    }

    #[test]
    fn en_passant_parsing_is_validated() {
        let mut e = Engine::with_hash_mb(1);
        e.set_fen("rnbqkbnr/pppp1ppp/8/8/3pP3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        assert_eq!(e.pos.current().en_passant_square, 20);
        e.set_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        assert_eq!(e.pos.current().en_passant_square, 0);
    }
}
