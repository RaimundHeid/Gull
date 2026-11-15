//! Basic chess types and bitboard helpers.
//!
//! The encoding follows the C++ implementation exactly:
//!
//! * squares are 0..64 with `a1 = 0`, `h1 = 7`, `a8 = 56`, `h8 = 63`;
//! * colours are `White = 0`, `Black = 1`;
//! * pieces use the C++ codes `Empty=0`, `White=0`, `Black=1`,
//!   `WhitePawn=2` ... `BlackKing=15`;
//! * moves are packed into an `i32` as `(from << 6) | to | flags | score`.
//!
//! Keeping the same representation is important: the search stores move
//! ordering scores in the upper 16 bits and compares the *signed* full move
//! value while selecting moves.

use std::fmt;

/// A bitboard.
pub type Bitboard = u64;
/// A square index, `0..64`.
pub type Square = usize;
/// A piece code, `0..16`.
pub type Piece = usize;
/// A colour, `White = 0`, `Black = 1`.
pub type Color = usize;
/// A packed move, including its ordering score in the upper 16 bits.
pub type Move = i32;

pub const WHITE: Color = 0;
pub const BLACK: Color = 1;

pub const EMPTY: Piece = 0;
pub const BLACK_COLOR: Piece = 1;
pub const WHITE_PAWN: Piece = 2;
pub const BLACK_PAWN: Piece = 3;
pub const WHITE_KNIGHT: Piece = 4;
pub const BLACK_KNIGHT: Piece = 5;
pub const WHITE_LIGHT: Piece = 6;
pub const BLACK_LIGHT: Piece = 7;
pub const WHITE_DARK: Piece = 8;
pub const BLACK_DARK: Piece = 9;
pub const WHITE_ROOK: Piece = 10;
pub const BLACK_ROOK: Piece = 11;
pub const WHITE_QUEEN: Piece = 12;
pub const BLACK_QUEEN: Piece = 13;
pub const WHITE_KING: Piece = 14;
pub const BLACK_KING: Piece = 15;

pub const EMPTY_BB: Bitboard = 0;
pub const FILLED: Bitboard = !EMPTY_BB;
pub const INTERIOR: Bitboard = 0x007E7E7E7E7E7E00;
pub const BOUNDARY: Bitboard = !INTERIOR;
pub const WHITE_AREA: Bitboard = 0x0000_0000_FFFF_FFFF;
pub const BLACK_AREA: Bitboard = !WHITE_AREA;
pub const LIGHT_AREA: Bitboard = 0x55AA_55AA_55AA_55AA;
pub const DARK_AREA: Bitboard = !LIGHT_AREA;
pub const FILE_A: Bitboard = 0x0101_0101_0101_0101;
pub const LINE_0: Bitboard = 0x0000_0000_0000_00FF;

pub const MAX_PLY: usize = 256;
pub const MAX_MOVES: usize = 230;
pub const STACK_SIZE: usize = 2048;

pub const MATE_VALUE: i32 = 32760;
pub const EVAL_VALUE: i32 = 30000;
pub const KPK_VALUE: i32 = 300;
pub const TB_MATE_VALUE: i32 = 30000;
pub const TB_CURSED_MATE_VALUE: i32 = 3;

pub const FLAG_CASTLING: i32 = 0x1000;
pub const FLAG_EP: i32 = 0x2000;
pub const FLAG_P_KNIGHT: i32 = 0x4000;
pub const FLAG_P_LIGHT: i32 = 0x6000;
pub const FLAG_P_DARK: i32 = 0x8000;
pub const FLAG_P_ROOK: i32 = 0xA000;
pub const FLAG_P_QUEEN: i32 = 0xC000;

pub const CAN_CASTLE_OO: u8 = 1;
#[allow(non_upper_case_globals)]
pub const CAN_CASTLE_oo: u8 = 2;
pub const CAN_CASTLE_OOO: u8 = 4;
#[allow(non_upper_case_globals)]
pub const CAN_CASTLE_ooo: u8 = 8;

pub const FLAG_UNUSUAL_MATERIAL: i32 = 1 << 30;

pub const FLAG_SINGLE_BISHOP_W: u8 = 1 << 0;
pub const FLAG_SINGLE_BISHOP_B: u8 = 1 << 1;
pub const FLAG_CALL_EVAL_ENDGAME_W: u8 = 1 << 2;
pub const FLAG_CALL_EVAL_ENDGAME_B: u8 = 1 << 3;

// Search data flags (GData.gen_flags).
pub const FLAG_SORT: u8 = 1 << 0;
pub const FLAG_NO_BC_SORT: u8 = 1 << 1;

// Search function `flags` argument.
pub const FLAG_HASH_CHECK: i32 = 1 << 20;
pub const FLAG_HALT_CHECK: i32 = 1 << 21;
pub const FLAG_CALL_EVALUATION: i32 = 1 << 22;
pub const FLAG_DISABLE_NULL: i32 = 1 << 23;
pub const FLAG_NEAT_SEARCH: i32 = FLAG_HASH_CHECK | FLAG_HALT_CHECK | FLAG_CALL_EVALUATION;
pub const FLAG_NO_KILLER_UPDATE: i32 = 1 << 24;
pub const FLAG_RETURN_BEST_MOVE: i32 = 1 << 25;

// Search stages.
pub const STAGE_SEARCH: u8 = 0;
pub const S_HASH_MOVE: u8 = 1;
pub const S_GOOD_CAP: u8 = 2;
pub const S_SPECIAL: u8 = 3;
pub const S_QUIET: u8 = 4;
pub const S_BAD_CAP: u8 = 5;
pub const S_NONE: u8 = 6;
pub const STAGE_EVASION: u8 = 7;
pub const E_HASH_MOVE: u8 = 8;
pub const E_EV: u8 = 9;
pub const E_NONE: u8 = 10;
pub const STAGE_RAZORING: u8 = 11;
pub const R_HASH_MOVE: u8 = 12;
pub const R_CAP: u8 = 13;
pub const R_CHECKS: u8 = 14;
pub const R_NONE: u8 = 15;

pub const fn stage_none_mask() -> u32 {
    (1 << S_NONE) | (1 << E_NONE) | (1 << R_NONE)
}

pub const STAGE_NONE: u32 = stage_none_mask();

// Hash flags (GEntry.flags).
pub const HASH_FLAG_NONE: u16 = 0;

/// `PieceType` from the C++ implementation.
pub const PIECE_TYPE: [usize; 16] = [0, 0, 0, 0, 1, 1, 2, 2, 2, 2, 3, 3, 4, 4, 5, 5];

/// Material codes used to build the material index.
pub const MAT_WP: i32 = 11664;
pub const MAT_BP: i32 = 104976;
pub const MAT_WN: i32 = 1296;
pub const MAT_BN: i32 = 3888;
pub const MAT_WL: i32 = 81;
pub const MAT_BL: i32 = 162;
pub const MAT_WD: i32 = 324;
pub const MAT_BD: i32 = 648;
pub const MAT_WR: i32 = 9;
pub const MAT_BR: i32 = 27;
pub const MAT_WQ: i32 = 1;
pub const MAT_BQ: i32 = 3;

pub const MAT_CODE: [i32; 16] = [
    0, 0, MAT_WP, MAT_BP, MAT_WN, MAT_BN, MAT_WL, MAT_BL, MAT_WD, MAT_BD, MAT_WR, MAT_BR, MAT_WQ,
    MAT_BQ, 0, 0,
];

pub const TOTAL_MAT: usize = (2 * (MAT_WQ as usize + MAT_BQ as usize)
    + MAT_WL as usize
    + MAT_BL as usize
    + MAT_WD as usize
    + MAT_BD as usize
    + 2 * (MAT_WR as usize + MAT_BR as usize + MAT_WN as usize + MAT_BN as usize)
    + 8 * (MAT_WP as usize + MAT_BP as usize))
    + 1;

pub const MAGIC_SIZE: usize = 107648;

/// Distances of a square from the centre, per axis.
pub const DIST_C: [i8; 8] = [3, 2, 1, 0, 0, 1, 2, 3];
/// Rank mapping used by the PST initialiser.
pub const RANK_R: [i8; 8] = [-3, -2, -1, 0, 1, 2, 3, 4];

pub const SEE_VALUE: [i32; 16] = [
    0, 0, 90, 90, 325, 325, 325, 325, 325, 325, 510, 510, 975, 975, 30000, 30000,
];

pub const PHASE: [i32; 5] = [0, 325, 325, 510, 975];
pub const MAX_PHASE: i32 =
    16 * PHASE[0] + 4 * PHASE[1] + 4 * PHASE[2] + 4 * PHASE[3] + 2 * PHASE[4];
pub const PHASE_MIN: i32 = 2 * PHASE[3] + PHASE[1] + PHASE[2];
pub const PHASE_MAX: i32 = MAX_PHASE - PHASE[1] - PHASE[2];

/// `DATA->UpdateCastling` from the C++ source.
pub const UPDATE_CASTLING: [u8; 64] = {
    let mut t = [0xFFu8; 64];
    t[0] = 0xFF ^ CAN_CASTLE_OOO;
    t[4] = 0xFF ^ (CAN_CASTLE_OO | CAN_CASTLE_OOO);
    t[7] = 0xFF ^ CAN_CASTLE_OO;
    t[56] = 0xFF ^ CAN_CASTLE_ooo;
    t[60] = 0xFF ^ (CAN_CASTLE_oo | CAN_CASTLE_ooo);
    t[63] = 0xFF ^ CAN_CASTLE_oo;
    t
};

#[inline(always)]
pub const fn sq(file: usize, rank: usize) -> Square {
    rank * 8 + file
}

#[inline(always)]
pub const fn file(s: Square) -> usize {
    s & 7
}

#[inline(always)]
pub const fn rank(s: Square) -> usize {
    s >> 3
}

/// Rank from the moving side's point of view.
#[inline(always)]
pub const fn crank(me: Color, s: Square) -> usize {
    if me == WHITE {
        rank(s)
    } else {
        7 - rank(s)
    }
}

#[inline(always)]
pub const fn ndiag(s: Square) -> usize {
    7 - file(s) + rank(s)
}

#[inline(always)]
pub const fn sdiag(s: Square) -> usize {
    file(s) + rank(s)
}

#[inline(always)]
pub fn dist(a: Square, b: Square) -> i32 {
    let r = (rank(a) as i32 - rank(b) as i32).abs();
    let f = (file(a) as i32 - file(b) as i32).abs();
    r.max(f)
}

#[inline(always)]
pub const fn bit(s: Square) -> Bitboard {
    1u64 << s
}

#[inline(always)]
pub const fn add_bb(x: &mut Bitboard, s: Square) {
    *x |= bit(s);
}

#[inline(always)]
pub const fn cut(x: &mut Bitboard) {
    *x &= x.wrapping_sub(1);
}

#[inline(always)]
pub const fn multiple(x: Bitboard) -> bool {
    (x & x.wrapping_sub(1)) != 0
}

#[inline(always)]
pub const fn single(x: Bitboard) -> bool {
    (x & x.wrapping_sub(1)) == 0
}

#[inline(always)]
pub const fn lsb(x: Bitboard) -> Square {
    x.trailing_zeros() as Square
}

#[inline(always)]
pub const fn msb(x: Bitboard) -> Square {
    (63 - x.leading_zeros()) as Square
}

#[inline(always)]
pub const fn popcount(x: Bitboard) -> i32 {
    x.count_ones() as i32
}

/// `NB(me, x)`: least significant bit for White, most significant for Black.
#[inline(always)]
pub const fn nb(me: Color, x: Bitboard) -> Square {
    if me == WHITE {
        lsb(x)
    } else {
        msb(x)
    }
}

/// `NBZ(me, x)`, with the C++ zero fallback (0 for White, 63 for Black).
#[inline(always)]
pub const fn nbz(me: Color, x: Bitboard) -> Square {
    if me == WHITE {
        if x != 0 {
            lsb(x)
        } else {
            0
        }
    } else if x != 0 {
        msb(x)
    } else {
        63
    }
}

#[inline(always)]
pub const fn lsbz(x: Bitboard) -> Square {
    if x != 0 {
        lsb(x)
    } else {
        0
    }
}

#[inline(always)]
pub const fn msbz(x: Bitboard) -> Square {
    if x != 0 {
        msb(x)
    } else {
        63
    }
}

/// `Compose(x, y)`: pack two signed 16-bit values into one `i32`.
#[inline(always)]
pub const fn compose(x: i32, y: i32) -> i32 {
    (x as u32).wrapping_add((y as u32) << 16) as i32
}

/// `Compose16(x, y)`.
#[inline(always)]
pub const fn compose16(x: i32, y: i32) -> i32 {
    compose(x / 16, y / 16)
}

/// `Compose256(x, y)`.
#[inline(always)]
pub const fn compose256(x: i32, y: i32) -> i32 {
    compose(x / 256, y / 256)
}

/// `Opening(x)`: sign-extended low 16 bits.
#[inline(always)]
pub const fn opening(x: i32) -> i32 {
    (x as u16 as i16) as i32
}

/// `Endgame(x)`: sign-extended high 16 bits plus the sign bit of bit 15.
#[inline(always)]
pub const fn endgame(x: i32) -> i32 {
    ((x >> 15) & 1) + (((x as u32 >> 16) as u16 as i16) as i32)
}

#[inline(always)]
pub const fn sgn(x: i32) -> i32 {
    if x == 0 {
        0
    } else if x > 0 {
        1
    } else {
        -1
    }
}

#[inline(always)]
pub const fn is_slider(p: Piece) -> bool {
    (0x3FC0u64 & (1u64 << p)) != 0
}

#[inline(always)]
pub const fn make_move(from: Square, to: Square) -> Move {
    ((from as i32) << 6) | (to as i32)
}

#[inline(always)]
pub const fn make_move_f(from: Square, to: Square, flags: i32) -> Move {
    ((from as i32) << 6) | (to as i32) | flags
}

#[inline(always)]
pub const fn make_move_fs(from: Square, to: Square, flags: i32, score: i32) -> Move {
    ((from as i32) << 6) | (to as i32) | flags | score
}

#[inline(always)]
pub const fn move_from(m: Move) -> Square {
    ((m >> 6) & 0x3f) as Square
}

#[inline(always)]
pub const fn move_to(m: Move) -> Square {
    (m & 0x3f) as Square
}

#[inline(always)]
pub const fn move_flags(m: Move) -> i32 {
    m & 0xF000
}

#[inline(always)]
pub const fn move_low(m: Move) -> Move {
    m & 0xFFFF
}

#[inline(always)]
pub const fn set_score(m: Move, score: i32) -> Move {
    (m & 0xFFFF) | (((score as u32) << 16) as i32)
}

#[inline(always)]
pub const fn get_score(m: Move) -> i32 {
    m >> 16
}

#[inline(always)]
pub const fn is_promotion(m: Move) -> bool {
    (m & 0xC000) != 0
}

#[inline(always)]
pub const fn is_castling(m: Move) -> bool {
    (m & 0x1000) != 0
}

#[inline(always)]
pub const fn is_ep(m: Move) -> bool {
    (m & 0xF000) == FLAG_EP
}

#[inline(always)]
pub const fn promotion_piece(m: Move, side: Color) -> Piece {
    side + (((m & 0xF000) >> 12) as usize)
}

#[inline(always)]
pub const fn ipawn(me: Color) -> Piece {
    WHITE_PAWN | me
}

#[inline(always)]
pub const fn iknight(me: Color) -> Piece {
    WHITE_KNIGHT | me
}

#[inline(always)]
pub const fn ilight(me: Color) -> Piece {
    WHITE_LIGHT | me
}

#[inline(always)]
pub const fn idark(me: Color) -> Piece {
    WHITE_DARK | me
}

#[inline(always)]
pub const fn irook(me: Color) -> Piece {
    WHITE_ROOK | me
}

#[inline(always)]
pub const fn iqueen(me: Color) -> Piece {
    WHITE_QUEEN | me
}

#[inline(always)]
pub const fn iking(me: Color) -> Piece {
    WHITE_KING | me
}

#[inline(always)]
pub const fn opp(me: Color) -> Color {
    1 ^ me
}

/// Piece letter for FEN output.
#[inline]
pub fn piece_char(p: Piece) -> char {
    match p {
        WHITE_PAWN => 'P',
        BLACK_PAWN => 'p',
        WHITE_KNIGHT => 'N',
        BLACK_KNIGHT => 'n',
        WHITE_LIGHT | WHITE_DARK => 'B',
        BLACK_LIGHT | BLACK_DARK => 'b',
        WHITE_ROOK => 'R',
        BLACK_ROOK => 'r',
        WHITE_QUEEN => 'Q',
        BLACK_QUEEN => 'q',
        WHITE_KING => 'K',
        BLACK_KING => 'k',
        _ => '.',
    }
}

/// Returns the piece code for a FEN letter, with the bishop colour based on
/// the square (as `get_board` does in the C++ source).
#[inline]
pub fn piece_from_char(c: u8, s: Square) -> Piece {
    match c {
        b'P' => WHITE_PAWN,
        b'N' => WHITE_KNIGHT,
        b'B' => {
            if sdiag(s) % 2 == 0 {
                WHITE_DARK
            } else {
                WHITE_LIGHT
            }
        }
        b'R' => WHITE_ROOK,
        b'Q' => WHITE_QUEEN,
        b'K' => WHITE_KING,
        b'p' => BLACK_PAWN,
        b'n' => BLACK_KNIGHT,
        b'b' => {
            if sdiag(s) % 2 == 0 {
                BLACK_DARK
            } else {
                BLACK_LIGHT
            }
        }
        b'r' => BLACK_ROOK,
        b'q' => BLACK_QUEEN,
        b'k' => BLACK_KING,
        _ => EMPTY,
    }
}

/// Convert a square to algebraic notation.
#[inline]
pub fn square_to_string(s: Square) -> String {
    format!(
        "{}{}",
        (b'a' + file(s) as u8) as char,
        (b'1' + rank(s) as u8) as char
    )
}

/// Convert an algebraic square (e.g. `"e4"`) to an index.
#[inline]
pub fn parse_square(s: &str) -> Option<Square> {
    let b = s.as_bytes();
    if b.len() < 2 {
        return None;
    }
    let f = b[0].checked_sub(b'a')? as usize;
    let r = b[1].checked_sub(b'1')? as usize;
    if f < 8 && r < 8 {
        Some(sq(f, r))
    } else {
        None
    }
}

/// `move_to_string` from the C++ source.
pub fn move_to_string(m: Move) -> String {
    let mut s = String::with_capacity(5);
    s.push((b'a' + ((m >> 6) & 7) as u8) as char);
    s.push((b'1' + ((m >> 9) & 7) as u8) as char);
    s.push((b'a' + (m & 7) as u8) as char);
    s.push((b'1' + ((m >> 3) & 7) as u8) as char);
    if is_promotion(m) {
        match m & 0xF000 {
            FLAG_P_QUEEN => s.push('q'),
            FLAG_P_ROOK => s.push('r'),
            FLAG_P_LIGHT | FLAG_P_DARK => s.push('b'),
            FLAG_P_KNIGHT => s.push('n'),
            _ => {}
        }
    }
    s
}

impl fmt::Display for MoveString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", move_to_string(self.0))
    }
}

/// Small wrapper used for pretty-printing moves in tests.
pub struct MoveString(pub Move);

/// Convert scores to the UCI `cp`/`mate` representation used by the original
/// implementation.
pub fn uci_score(score: i32) -> (&'static str, i32) {
    if score > EVAL_VALUE {
        ("mate", (MATE_VALUE - score + 1) / 2)
    } else if score < -EVAL_VALUE {
        ("mate", -(score + MATE_VALUE + 1) / 2)
    } else {
        ("cp", score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_helpers() {
        assert_eq!(sq(4, 0), 4);
        assert_eq!(file(27), 3);
        assert_eq!(rank(27), 3);
        assert_eq!(square_to_string(27), "d4");
        assert_eq!(parse_square("d4"), Some(27));
        assert_eq!(parse_square("a1"), Some(0));
        assert_eq!(parse_square("h8"), Some(63));
        assert_eq!(parse_square("i9"), None);
    }

    #[test]
    fn compose_and_open_endgame_roundtrip() {
        for &(a, b) in &[(0, 0), (10, -20), (-300, 400), (32767, -32768), (-1, 1)] {
            let c = compose(a, b);
            assert_eq!(opening(c), a);
            assert_eq!(endgame(c), b);
        }
        assert_eq!(compose16(160, -160), compose(10, -10));
        assert_eq!(compose256(512, 256), compose(2, 1));
    }

    #[test]
    fn bit_helpers() {
        assert_eq!(bit(0), 1);
        assert_eq!(bit(63), 1u64 << 63);
        assert_eq!(lsb(0b1000), 3);
        assert_eq!(msb(0b1000), 3);
        assert_eq!(popcount(0xFF), 8);
        assert_eq!(nb(WHITE, 0b101000), 3);
        assert_eq!(nb(BLACK, 0b101000), 5);
        assert_eq!(nbz(WHITE, 0), 0);
        assert_eq!(nbz(BLACK, 0), 63);
        assert!(single(bit(5)));
        assert!(single(0));
        assert!(multiple(0b11));
        assert!(!multiple(bit(2)));
    }

    #[test]
    fn move_packing() {
        let m = make_move_fs(4, 12, 0, 0x1234 << 16);
        assert_eq!(move_from(m), 4);
        assert_eq!(move_to(m), 12);
        assert_eq!(move_low(m), make_move(4, 12));
        assert_eq!(get_score(m), 0x1234);
        assert_eq!(set_score(make_move(4, 12), -5) >> 16, -5);
        assert_eq!(move_to_string(make_move(4, 12)), "e1e2");
        assert_eq!(move_to_string(make_move_f(4, 6, FLAG_CASTLING)), "e1g1");
        assert_eq!(move_to_string(make_move_f(52, 60, FLAG_P_QUEEN)), "e7e8q");
        assert!(is_castling(make_move_f(4, 6, FLAG_CASTLING)));
        assert!(is_ep(make_move_f(36, 44, FLAG_EP)));
        assert!(is_promotion(make_move_f(52, 60, FLAG_P_KNIGHT)));
        assert_eq!(
            promotion_piece(make_move_f(52, 60, FLAG_P_KNIGHT), WHITE),
            WHITE_KNIGHT
        );
        assert_eq!(
            promotion_piece(make_move_f(52, 60, FLAG_P_QUEEN), BLACK),
            BLACK_QUEEN
        );
    }

    #[test]
    fn piece_codes() {
        assert_eq!(ipawn(WHITE), WHITE_PAWN);
        assert_eq!(ipawn(BLACK), BLACK_PAWN);
        assert_eq!(iking(WHITE), WHITE_KING);
        assert_eq!(iking(BLACK), BLACK_KING);
        assert!(is_slider(WHITE_LIGHT));
        assert!(is_slider(BLACK_QUEEN));
        assert!(!is_slider(WHITE_KNIGHT));
        assert!(!is_slider(WHITE_KING));
        assert_eq!(piece_from_char(b'B', sq(1, 0)), WHITE_LIGHT);
        assert_eq!(piece_from_char(b'B', sq(0, 0)), WHITE_DARK);
        assert_eq!(piece_from_char(b'b', sq(1, 0)), BLACK_LIGHT);
        assert_eq!(piece_from_char(b'b', sq(0, 0)), BLACK_DARK);
        for p in [WHITE_PAWN, BLACK_QUEEN, WHITE_KING] {
            assert_eq!(
                piece_char(p),
                match p {
                    WHITE_PAWN => 'P',
                    BLACK_QUEEN => 'q',
                    _ => 'K',
                }
            );
        }
    }

    #[test]
    fn uci_score_conversion() {
        assert_eq!(uci_score(0), ("cp", 0));
        assert_eq!(uci_score(30000), ("cp", 30000));
        assert_eq!(uci_score(MATE_VALUE - 1), ("mate", 1));
        assert_eq!(uci_score(MATE_VALUE - 3), ("mate", 2));
        assert_eq!(uci_score(-(MATE_VALUE - 1)), ("mate", -1));
        assert_eq!(uci_score(-(MATE_VALUE - 3)), ("mate", -2));
    }

    #[test]
    fn update_castling_table() {
        assert_eq!(UPDATE_CASTLING[0], 0xFF ^ CAN_CASTLE_OOO);
        assert_eq!(UPDATE_CASTLING[4], 0xFF ^ (CAN_CASTLE_OO | CAN_CASTLE_OOO));
        assert_eq!(UPDATE_CASTLING[7], 0xFF ^ CAN_CASTLE_OO);
        assert_eq!(UPDATE_CASTLING[56], 0xFF ^ CAN_CASTLE_ooo);
        assert_eq!(UPDATE_CASTLING[60], 0xFF ^ (CAN_CASTLE_oo | CAN_CASTLE_ooo));
        assert_eq!(UPDATE_CASTLING[63], 0xFF ^ CAN_CASTLE_oo);
        assert_eq!(UPDATE_CASTLING[12], 0xFF);
    }

    #[test]
    fn total_material_matches_cpp() {
        // The C++ TotalMat macro evaluates to 944784.
        assert_eq!(TOTAL_MAT, 944784);
    }
}
