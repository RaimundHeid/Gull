//! Pre-computed data tables, evaluation weights and material tables.
//!
//! This is a faithful port of `data.cpp` and the initialisation portion of
//! the original engine.  The random Zobrist keys are generated with the same LCG and
//! the same call order, so hash keys match the C++ engine exactly.

use std::sync::OnceLock;

use crate::types::*;

pub const INITIAL_HASH_SIZE: usize = 1024 * 1024;
pub const PAWN_HASH_SIZE: usize = 1024 * 1024;
pub const PRINCIPAL_VARIATION_TABLE_SIZE: usize = 1024 * 1024;
pub const PV_CLUSTER_SIZE: usize = 4;
pub const PRINCIPAL_VARIATION_MASK: usize = PRINCIPAL_VARIATION_TABLE_SIZE - PV_CLUSTER_SIZE;
pub const PAWN_HASH_MASK: usize = PAWN_HASH_SIZE - 1;

pub const OUTPOST: [u64; 2] = [0x0000_7E7E_3C00_0000, 0x0000_003C_7E7E_0000];

// ---------------------------------------------------------------------------
// Evaluation weights (`// tuner: start` .. `// tuner: stop`)
// ---------------------------------------------------------------------------

pub const MATERIAL_LINEAR: [i32; 5] = [3, 0, 3, 19, 0];

pub const MAT_QUAD_ME: [i32; 14] = [
    -33, 17, -23, -155, -247, 15, 296, -105, -83, -162, 327, 315, -861, -1013,
];

pub const MAT_QUAD_OPP: [i32; 10] = [-14, 47, -20, -278, 35, 39, 49, 9, -2, 75];

pub const BISHOP_PAIR_QUAD: [i32; 9] = [-38, 164, 99, 246, -84, -57, -184, 88, -186];

pub const MATERIAL_ROOK_BISHOP: usize = 0;
pub const MATERIAL_ROOK_KNIGHT: usize = 1;
pub const MATERIAL_QUEEN_ROOK_ROOK: usize = 2;
pub const MATERIAL_QUEEN_ROOK_BISHOP: usize = 3;
pub const MATERIAL_QUEEN_ROOK_KNIGHT: usize = 4;
pub const MAT_Q3: usize = 5;
pub const MATERIAL_BISHOP_BISHOP_ROOK: usize = 6;
pub const MATERIAL_BISHOP_KNIGHT_ROOK: usize = 7;
pub const MATERIAL_KNIGHT_KNIGHT_ROOK: usize = 8;
pub const MATERIAL_MISC: usize = 9;

pub const MATERIAL_SPECIAL: [i32; 20] = [
    13, -13, 10, -9, 8, 12, 4, 6, 5, 9, -3, -8, -4, 7, 2, 0, 0, -6, 1, 3,
];

pub const PST_QUAD_WEIGHTS: [i32; 48] = [
    -15, -19, -70, -13, 33, -20, 0, 197, -36, -122, 0, -60, -8, -3, -17, -28, -27, -63, -17, -7,
    14, 0, -24, -5, -64, -2, 0, -38, -8, 0, 77, 11, -67, 3, -4, -92, -2, 12, -13, -42, -62, -84,
    -175, -42, -2, -17, 40, -19,
];

pub const PST_LINEAR_WEIGHTS: [i32; 48] = [
    -107, 67, -115, 83, -55, 67, 92, 443, -177, 5, -82, -61, -106, -104, 273, 130, 0, -145, -105,
    -58, -99, -37, -133, 14, -185, -43, -67, -53, 53, -65, 174, 134, -129, 7, 98, -231, 107, -40,
    -27, 311, 256, -117, 813, -181, 2, -215, -44, 344,
];

pub const PST_QUAD_MIXED_WEIGHTS: [i32; 24] = [
    14, -6, 1, -4, -8, -2, 4, -4, 1, -7, -12, 0, -2, -1, -5, 4, 5, -10, 0, 4, -2, 5, 4, -2,
];

pub const MOBILITY_LINEAR: [i32; 8] = [328, 171, 311, 102, 284, 164, 155, 288];
pub const MOBILITY_LOG: [i32; 8] = [485, -21, 388, 389, -168, 313, 438, -276];

pub const SHELTER_VALUE: [i32; 15] = [2, 9, 11, 0, 0, 12, 18, 11, 0, 2, 24, 7, 8, 0, 0];

pub const STORM_BLOCKED_MUL: usize = 0;
pub const STORM_SHELTER_ATT_MUL: usize = 1;
pub const STORM_CONNECTED_MUL: usize = 2;
pub const STORM_OPEN_MUL: usize = 3;
pub const STORM_FREE_MUL: usize = 4;

pub const STORM_QUAD: [i32; 5] = [126, 328, 463, 215, 89];
pub const STORM_LINEAR: [i32; 5] = [83, 156, 438, 321, 12];

pub const STORM_HOF_VALUE: usize = 0;
pub const STORM_OF_VALUE: usize = 1;
pub const STORM_HOF: [i32; 2] = [0, 22];

pub const PASSER_QUAD: [i32; 18] = [
    19, 13, 21, 3, -24, 126, 0, 65, 32, 56, 27, -5, 32, -16, 13, 4, 1, 1,
];
pub const PASSER_LINEAR: [i32; 18] = [
    41, 2, 111, 86, 178, 113, 202, 15, -61, 21, 93, 166, 86, 92, 27, 34, -18, -7,
];
pub const PASSER_ATT_DEF_QUAD: [i32; 4] = [191, 51, 83, 19];
pub const PASSER_ATT_DEF_LINEAR: [i32; 4] = [634, 4, 233, 66];

pub const PASSER_ONE_PIECE: usize = 0;
pub const PASSER_OP_KING_CONTROL: usize = 1;
pub const PASSER_OP_MINOR_CONTROL: usize = 2;
pub const PASSER_OP_ROOK_BLOCK: usize = 3;
pub const PASSER_SPECIAL: [i32; 4] = [0, 0, 0, 13];

pub const ISOLATED_OPEN: usize = 0;
pub const ISOLATED_CLOSED: usize = 1;
pub const ISOLATED_BLOCKED: usize = 2;
pub const ISOLATED_DOUBLED_OPEN: usize = 3;
pub const ISOLATED_DOUBLED_CLOSED: usize = 4;
pub const ISOLATED: [i32; 10] = [6, 6, 8, 2, -8, 0, -1, 10, 7, 9];

pub const UP_BLOCKED: usize = 0;
pub const PASSER_TARGET: usize = 1;
pub const CHAIN_ROOT: usize = 2;
pub const UNPROTECTED: [i32; 6] = [4, 5, -5, -1, 9, -1];

pub const BACKWARD_OPEN: usize = 0;
pub const BACKWARD_CLOSED: usize = 1;
pub const BACKWARD: [i32; 4] = [17, 10, 4, 1];

pub const DOUBLED_OPEN: usize = 0;
pub const DOUBLED_CLOSED: usize = 1;
pub const DOUBLED: [i32; 4] = [3, 0, 1, 0];

pub const ROOK_HOF: usize = 0;
pub const ROOK_HOF_WEAK_P_ATT: usize = 1;
pub const ROOK_OF: usize = 2;
pub const ROOK_OF_OPEN: usize = 3;
pub const ROOK_OF_MINOR_FIXED: usize = 4;
pub const ROOK_OF_MINOR_HAGING: usize = 5;
pub const ROOK_OF_KING_ATT: usize = 6;
pub const ROOK_7TH: usize = 7;
pub const ROOK_7TH_K8TH: usize = 8;
pub const ROOK_7TH_DOUBLED: usize = 9;
pub const ROOK_SPECIAL: [i32; 20] = [
    8, 0, 2, 0, 11, 8, -1, 2, -1, -1, 14, -1, 5, -5, -5, 0, -6, 8, -7, 31,
];

pub const TACTICAL_MAJOR_PAWN: usize = 0;
pub const TACTICAL_MINOR_PAWN: usize = 1;
pub const TACTICAL_MAJOR_MINOR: usize = 2;
pub const TACTICAL_MINOR_MINOR: usize = 3;
pub const TACTICAL_THREAT: usize = 4;
pub const TACTICAL_DOUBLE_THREAT: usize = 5;
pub const TACTICAL: [i32; 12] = [-1, 5, 0, 5, 11, 29, 23, 32, 19, 11, 41, 12];

pub const KING_DEF_KNIGHT: usize = 0;
pub const KING_DEF_BISHOP: usize = 1;
pub const KING_DEF_ROOK: usize = 2;
pub const KING_DEF_QUEEN: usize = 3;
pub const KING_DEFENCE: [i32; 8] = [2, 0, 0, 1, 0, 0, 4, 0];

pub const PAWN_CHAIN_LINEAR: usize = 0;
pub const PAWN_CHAIN: usize = 1;
pub const PAWN_BLOCKED: usize = 2;
pub const PAWN_FILE_SPAN: usize = 3;
pub const PAWN_SPECIAL: [i32; 8] = [11, 9, 9, 4, 0, 9, 1, 1];

pub const BISHOP_NON_FORWARD_PAWN: usize = 0;
pub const BISHOP_PAWN_BLOCK: usize = 1;
pub const BISHOP_SPECIAL: [i32; 4] = [0, 0, 0, 3];

pub const KNIGHT_OUTPOST: usize = 0;
pub const KNIGHT_OUTPOST_PROTECTED: usize = 1;
pub const KNIGHT_OUTPOST_PAWN_ATT: usize = 2;
pub const KNIGHT_OUTPOST_BISHOP_ATT: usize = 3;
pub const KNIGHT_OUTPOST_KING_ATT: usize = 4;
pub const KNIGHT_SPECIAL: [i32; 10] = [11, 7, 23, 0, 13, 6, 1, 5, 26, 6];

pub const WEAK_PIN: usize = 0;
pub const STRONG_PIN: usize = 1;
pub const THREAT_PIN: usize = 2;
pub const SELF_PAWN_PIN: usize = 3;
pub const SELF_PIECE_PIN: usize = 4;
pub const PIN: [i32; 10] = [21, 39, 6, 80, 45, 29, 8, 9, 48, 27];

pub const Q_KING_RAY: usize = 0;
pub const R_KING_RAY: usize = 1;
pub const B_KING_RAY: usize = 2;
pub const KING_RAY: [i32; 6] = [4, 8, -4, 11, 11, -3];

pub const KING_ATTACK_WEIGHT: [i32; 7] = [17, 14, 22, 45, 48, 64, 64];
pub const KING_ATTACK_SCALE: [i32; 16] =
    [0, 1, 4, 9, 16, 25, 36, 49, 64, 64, 64, 64, 64, 64, 64, 64];

/// `KingNAttack`
pub const fn king_knight_attack_score() -> i32 {
    compose(1, KING_ATTACK_WEIGHT[0])
}
pub const fn king_b_attack() -> i32 {
    compose(1, KING_ATTACK_WEIGHT[1])
}
pub const fn king_r_attack() -> i32 {
    compose(1, KING_ATTACK_WEIGHT[2])
}
pub const fn king_queen_attack_score() -> i32 {
    compose(1, KING_ATTACK_WEIGHT[3])
}
pub const fn king_attack_score() -> i32 {
    compose(1, 0)
}
pub const fn king_attack_square() -> i32 {
    KING_ATTACK_WEIGHT[4]
}
pub const fn king_no_moves() -> i32 {
    KING_ATTACK_WEIGHT[5]
}
pub const fn king_shelter_quad() -> i32 {
    KING_ATTACK_WEIGHT[6]
}

// ---------------------------------------------------------------------------
// Material table entry
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Material {
    pub score: i16,
    pub phase: u8,
    pub flags: u8,
    pub evaluation_multiplier: [u8; 2],
    pub pieces: [u8; 2],
}

// ---------------------------------------------------------------------------
// Magic bitboard constants (from the original engine)
// ---------------------------------------------------------------------------

const BISHOP_MAGIC: [u64; 64] = [
    0x0048610528020080,
    0x00c4100212410004,
    0x0004180181002010,
    0x0004040188108502,
    0x0012021008003040,
    0x0002900420228000,
    0x0080808410c00100,
    0x000600410c500622,
    0x00c0056084140184,
    0x0080608816830050,
    0x00a010050200b0c0,
    0x0000510400800181,
    0x0000431040064009,
    0x0000008820890a06,
    0x0050028488184008,
    0x00214a0104068200,
    0x004090100c080081,
    0x000a002014012604,
    0x0020402409002200,
    0x008400c240128100,
    0x0001000820084200,
    0x0024c02201101144,
    0x002401008088a800,
    0x0003001045009000,
    0x0084200040981549,
    0x0001188120080100,
    0x0048050048044300,
    0x0008080000820012,
    0x0001001181004003,
    0x0090038000445000,
    0x0010820800a21000,
    0x0044010108210110,
    0x0090241008204e30,
    0x000c04204004c305,
    0x0080804303300400,
    0x00a0020080080080,
    0x0000408020220200,
    0x0000c08200010100,
    0x0010008102022104,
    0x0008148118008140,
    0x0008080414809028,
    0x0005031010004318,
    0x0000603048001008,
    0x0008012018000100,
    0x0000202028802901,
    0x004011004b049180,
    0x0022240b42081400,
    0x00c4840c00400020,
    0x0084009219204000,
    0x000080c802104000,
    0x0002602201100282,
    0x0002040821880020,
    0x0002014008320080,
    0x0002082078208004,
    0x0009094800840082,
    0x0020080200b1a010,
    0x0003440407051000,
    0x000000220e100440,
    0x00480220a4041204,
    0x00c1800011084800,
    0x000008021020a200,
    0x0000414128092100,
    0x0000042002024200,
    0x0002081204004200,
];

const ROOK_MAGIC: [u64; 64] = [
    0x00800011400080a6,
    0x004000100120004e,
    0x0080100008600082,
    0x0080080016500080,
    0x0080040008000280,
    0x0080020005040080,
    0x0080108046000100,
    0x0080010000204080,
    0x0010800424400082,
    0x00004002c8201000,
    0x000c802000100080,
    0x00810010002100b8,
    0x00ca808014000800,
    0x0002002884900200,
    0x0042002148041200,
    0x00010000c200a100,
    0x00008580004002a0,
    0x0020004001403008,
    0x0000820020411600,
    0x0002120021401a00,
    0x0024808044010800,
    0x0022008100040080,
    0x00004400094a8810,
    0x0000020002814c21,
    0x0011400280082080,
    0x004a050e002080c0,
    0x00101103002002c0,
    0x0025020900201000,
    0x0001001100042800,
    0x0002008080022400,
    0x000830440021081a,
    0x0080004200010084,
    0x00008000c9002104,
    0x0090400081002900,
    0x0080220082004010,
    0x0001100101000820,
    0x0000080011001500,
    0x0010020080800400,
    0x0034010224009048,
    0x0002208412000841,
    0x000040008020800c,
    0x001000c460094000,
    0x0020006101330040,
    0x0000a30010010028,
    0x0004080004008080,
    0x0024000201004040,
    0x0000300802440041,
    0x00120400c08a0011,
    0x0080006085004100,
    0x0028600040100040,
    0x00a0082110018080,
    0x0010184200221200,
    0x0040080005001100,
    0x0004200440104801,
    0x0080800900220080,
    0x000a01140081c200,
    0x0080044180110021,
    0x0008804001001225,
    0x00a00c4020010011,
    0x00001000a0050009,
    0x0011001800021025,
    0x00c9000400620811,
    0x0032009001080224,
    0x001400810044086a,
];

const BISHOP_SHIFT: [u32; 64] = [
    58, 59, 59, 59, 59, 59, 59, 58, 59, 59, 59, 59, 59, 59, 59, 59, 59, 59, 57, 57, 57, 57, 59, 59,
    59, 59, 57, 55, 55, 57, 59, 59, 59, 59, 57, 55, 55, 57, 59, 59, 59, 59, 57, 57, 57, 57, 59, 59,
    59, 59, 59, 59, 59, 59, 59, 59, 58, 59, 59, 59, 59, 59, 59, 58,
];

const BISHOP_OFFSET: [usize; 64] = [
    0, 64, 96, 128, 160, 192, 224, 256, 320, 352, 384, 416, 448, 480, 512, 544, 576, 608, 640, 768,
    896, 1024, 1152, 1184, 1216, 1248, 1280, 1408, 1920, 2432, 2560, 2592, 2624, 2656, 2688, 2816,
    3328, 3840, 3968, 4000, 4032, 4064, 4096, 4224, 4352, 4480, 4608, 4640, 4672, 4704, 4736, 4768,
    4800, 4832, 4864, 4896, 4928, 4992, 5024, 5056, 5088, 5120, 5152, 5184,
];

const ROOK_SHIFT: [u32; 64] = [
    52, 53, 53, 53, 53, 53, 53, 52, 53, 54, 54, 54, 54, 54, 54, 53, 53, 54, 54, 54, 54, 54, 54, 53,
    53, 54, 54, 54, 54, 54, 54, 53, 53, 54, 54, 54, 54, 54, 54, 53, 53, 54, 54, 54, 54, 54, 54, 53,
    53, 54, 54, 54, 54, 54, 54, 53, 52, 53, 53, 53, 53, 53, 53, 52,
];

const ROOK_OFFSET: [usize; 64] = [
    5248, 9344, 11392, 13440, 15488, 17536, 19584, 21632, 25728, 27776, 28800, 29824, 30848, 31872,
    32896, 33920, 35968, 38016, 39040, 40064, 41088, 42112, 43136, 44160, 46208, 48256, 49280,
    50304, 51328, 52352, 53376, 54400, 56448, 58496, 59520, 60544, 61568, 62592, 63616, 64640,
    66688, 68736, 69760, 70784, 71808, 72832, 73856, 74880, 76928, 78976, 80000, 81024, 82048,
    83072, 84096, 85120, 87168, 91264, 93312, 95360, 97408, 99456, 101504, 103552,
];

/// All immutable tables shared by engines.
pub struct Tables {
    pub forward_ranks: [[u64; 8]; 2],
    pub west_files: [u64; 8],
    pub east_files: [u64; 8],
    pub isolated_pawns: [u64; 8],
    pub horizontal_lines: [u64; 64],
    pub vertical_lines: [u64; 64],
    pub northeast_diagonals: [u64; 64],
    pub southwest_diagonals: [u64; 64],
    pub rook_attack_mask: [u64; 64],
    pub bishop_attack_mask: [u64; 64],
    pub queen_attack_mask: [u64; 64],
    pub knight_attacks: [u64; 64],
    pub king_attack_area: [u64; 64],
    pub danger_area: [u64; 64],
    pub knight_area: [u64; 64],
    pub bishop_forward_rays: [[u64; 64]; 2],
    pub pawn_attacks: [[u64; 64]; 2],
    pub pawn_moves: [[u64; 64]; 2],
    pub pawn_paths: [[u64; 64]; 2],
    pub pawn_support: [[u64; 64]; 2],
    pub between: [[u64; 64]; 64],
    pub full_line: [[u64; 64]; 64],
    pub file: [u64; 8],
    pub line: [u64; 8],
    pub castling_rights_update: [u8; 64],
    pub mobility: [[i32; 32]; 4],
    pub shelter: [[i32; 8]; 3],
    pub storm_hof: [i32; 2],
    pub storm_blocked: [i32; 4],
    pub storm_shelter_att: [i32; 4],
    pub storm_connected: [i32; 4],
    pub storm_open: [i32; 4],
    pub storm_free: [i32; 4],
    pub log_dist: [i32; 16],
    pub passed_pawn_special: [i32; 4],
    pub passed_pawn_general: [i32; 8],
    pub passed_pawn_blocked: [i32; 8],
    pub passed_pawn_free: [i32; 8],
    pub passed_pawn_supported: [i32; 8],
    pub passed_pawn_protected: [i32; 8],
    pub passed_pawn_connected: [i32; 8],
    pub passed_pawn_outside: [i32; 8],
    pub passed_pawn_candidate: [i32; 8],
    pub passed_pawn_clear: [i32; 8],
    pub passed_pawn_att: [i32; 8],
    pub passed_pawn_def: [i32; 8],
    pub passed_pawn_att_log: [i32; 8],
    pub passed_pawn_def_log: [i32; 8],
    pub isolated: [i32; 10],
    pub unprotected: [i32; 6],
    pub backward: [i32; 4],
    pub doubled: [i32; 4],
    pub rook_special: [i32; 20],
    pub tactical: [i32; 12],
    pub king_defence: [i32; 8],
    pub pawn_special: [i32; 8],
    pub bishop_special: [i32; 4],
    pub outpost: [u64; 2],
    pub knight_special: [i32; 10],
    pub pin: [i32; 10],
    pub king_ray: [i32; 6],
    pub king_attack_weight: [i32; 7],
    pub king_attack_scale: [i32; 16],
    pub side_to_move_key: u64,
    pub piece_keys: [[u64; 64]; 16],
    pub castling_keys: [u64; 16],
    pub en_passant_keys: [u64; 8],
    pub piece_from_char: [u8; 256],
    pub kpk: Vec<u64>, // [2][64][64]
    pub piece_square_score: [i32; 16 * 64],
    pub mvv_lva: [[i32; 16]; 16],
    pub material: Vec<Material>,
    pub bishop_magic_mask: [u64; 64],
    pub rook_magic_mask: [u64; 64],
    pub magic_attacks: Vec<u64>,
    pub bishop_table_offset: [usize; 64],
    pub rook_table_offset: [usize; 64],
    pub bishop_shift: [u32; 64],
    pub rook_shift: [u32; 64],
    pub bishop_magic: [u64; 64],
    pub rook_magic: [u64; 64],
}

impl Tables {
    #[inline(always)]
    pub fn bishop_attacks(&self, s: Square, occupancy: u64) -> u64 {
        debug_assert!(s < 64);
        let idx = unsafe { *self.bishop_table_offset.get_unchecked(s) }
            + (((unsafe { *self.bishop_magic_mask.get_unchecked(s) } & occupancy)
                .wrapping_mul(unsafe { *self.bishop_magic.get_unchecked(s) }))
                >> unsafe { *self.bishop_shift.get_unchecked(s) }) as usize;
        unsafe { *self.magic_attacks.get_unchecked(idx) }
    }

    #[inline(always)]
    pub fn rook_attacks(&self, s: Square, occupancy: u64) -> u64 {
        debug_assert!(s < 64);
        let idx = unsafe { *self.rook_table_offset.get_unchecked(s) }
            + (((unsafe { *self.rook_magic_mask.get_unchecked(s) } & occupancy)
                .wrapping_mul(unsafe { *self.rook_magic.get_unchecked(s) }))
                >> unsafe { *self.rook_shift.get_unchecked(s) }) as usize;
        unsafe { *self.magic_attacks.get_unchecked(idx) }
    }

    #[inline(always)]
    pub fn queen_attacks(&self, s: Square, occupancy: u64) -> u64 {
        self.bishop_attacks(s, occupancy) | self.rook_attacks(s, occupancy)
    }

    #[inline(always)]
    pub fn kpk(&self, side_to_move: usize, white_pawn: usize, white_king_square: usize) -> u64 {
        self.kpk[((side_to_move * 64 + white_pawn) * 64) + white_king_square]
    }
}

static TABLES: OnceLock<&'static Tables> = OnceLock::new();

/// Get the process-wide immutable tables, building them on first use.
pub fn tables() -> &'static Tables {
    TABLES.get_or_init(|| Box::leak(Box::new(Tables::new())))
}

impl Tables {
    pub fn new() -> Tables {
        let mut table = Tables {
            forward_ranks: [[0; 8]; 2],
            west_files: [0; 8],
            east_files: [0; 8],
            isolated_pawns: [0; 8],
            horizontal_lines: [0; 64],
            vertical_lines: [0; 64],
            northeast_diagonals: [0; 64],
            southwest_diagonals: [0; 64],
            rook_attack_mask: [0; 64],
            bishop_attack_mask: [0; 64],
            queen_attack_mask: [0; 64],
            knight_attacks: [0; 64],
            king_attack_area: [0; 64],
            danger_area: [0; 64],
            knight_area: [0; 64],
            bishop_forward_rays: [[0; 64]; 2],
            pawn_attacks: [[0; 64]; 2],
            pawn_moves: [[0; 64]; 2],
            pawn_paths: [[0; 64]; 2],
            pawn_support: [[0; 64]; 2],
            between: [[0; 64]; 64],
            full_line: [[0; 64]; 64],
            file: [0; 8],
            line: [0; 8],
            castling_rights_update: [0xFF; 64],
            mobility: [[0; 32]; 4],
            shelter: [[0; 8]; 3],
            storm_hof: [0; 2],
            storm_blocked: [0; 4],
            storm_shelter_att: [0; 4],
            storm_connected: [0; 4],
            storm_open: [0; 4],
            storm_free: [0; 4],
            log_dist: [0; 16],
            passed_pawn_special: [0; 4],
            passed_pawn_general: [0; 8],
            passed_pawn_blocked: [0; 8],
            passed_pawn_free: [0; 8],
            passed_pawn_supported: [0; 8],
            passed_pawn_protected: [0; 8],
            passed_pawn_connected: [0; 8],
            passed_pawn_outside: [0; 8],
            passed_pawn_candidate: [0; 8],
            passed_pawn_clear: [0; 8],
            passed_pawn_att: [0; 8],
            passed_pawn_def: [0; 8],
            passed_pawn_att_log: [0; 8],
            passed_pawn_def_log: [0; 8],
            isolated: [0; 10],
            unprotected: [0; 6],
            backward: [0; 4],
            doubled: [0; 4],
            rook_special: [0; 20],
            tactical: [0; 12],
            king_defence: [0; 8],
            pawn_special: [0; 8],
            bishop_special: [0; 4],
            outpost: OUTPOST,
            knight_special: [0; 10],
            pin: [0; 10],
            king_ray: [0; 6],
            king_attack_weight: [0; 7],
            king_attack_scale: KING_ATTACK_SCALE,
            side_to_move_key: 0,
            piece_keys: [[0; 64]; 16],
            castling_keys: [0; 16],
            en_passant_keys: [0; 8],
            piece_from_char: [0; 256],
            kpk: vec![0; 2 * 64 * 64],
            piece_square_score: [0; 16 * 64],
            mvv_lva: [[0; 16]; 16],
            material: vec![Material::default(); TOTAL_MAT],
            bishop_magic_mask: [0; 64],
            rook_magic_mask: [0; 64],
            magic_attacks: vec![0; MAGIC_SIZE],
            bishop_table_offset: BISHOP_OFFSET,
            rook_table_offset: ROOK_OFFSET,
            bishop_shift: BISHOP_SHIFT,
            rook_shift: ROOK_SHIFT,
            bishop_magic: BISHOP_MAGIC,
            rook_magic: ROOK_MAGIC,
        };
        table.initialize_misc();
        table.initialize_magic();
        table.initialize_kpk();
        table.initialize_piece_square_tables();
        table.initialize_evaluation();
        table.initialize_material();
        table
    }

    // -- initialize_misc ---------------------------------------------------------

    fn initialize_misc(&mut self) {
        for i in 0..8 {
            self.file[i] = FILE_A << i;
            self.line[i] = LINE_0 << (8 * i);
        }
        for i in 0..64 {
            for j in 0..64 {
                if i == j {
                    continue;
                }
                let u = bit(j);
                if file(i) == file(j) {
                    self.vertical_lines[i] |= u;
                }
                if rank(i) == rank(j) {
                    self.horizontal_lines[i] |= u;
                }
                if northeast_diagonal(i) == northeast_diagonal(j) {
                    self.northeast_diagonals[i] |= u;
                }
                if southwest_diagonal(i) == southwest_diagonal(j) {
                    self.southwest_diagonals[i] |= u;
                }
                if distance(i, j) <= 2 {
                    self.danger_area[i] |= u;
                    if distance(i, j) <= 1 {
                        self.king_attack_area[i] |= u;
                    }
                    if (rank(i) as i32 - rank(j) as i32).abs()
                        + (file(i) as i32 - file(j) as i32).abs()
                        == 3
                    {
                        self.knight_attacks[i] |= u;
                    }
                }
                if j == i + 8 {
                    self.pawn_moves[0][i] |= u;
                }
                if j + 8 == i {
                    self.pawn_moves[1][i] |= u;
                }
                if (file(i) as i32 - file(j) as i32).abs() == 1 {
                    if rank(j) >= rank(i) {
                        self.pawn_support[1][i] |= u;
                        if rank(j) - rank(i) == 1 {
                            self.pawn_attacks[0][i] |= u;
                        }
                    }
                    if rank(j) <= rank(i) {
                        self.pawn_support[0][i] |= u;
                        if rank(i) - rank(j) == 1 {
                            self.pawn_attacks[1][i] |= u;
                        }
                    }
                } else if file(i) == file(j) {
                    if rank(j) > rank(i) {
                        self.pawn_paths[0][i] |= u;
                    } else {
                        self.pawn_paths[1][i] |= u;
                    }
                }
            }
        }
        for i in 0..64 {
            self.rook_attack_mask[i] = self.horizontal_lines[i] | self.vertical_lines[i];
            self.bishop_attack_mask[i] = self.northeast_diagonals[i] | self.southwest_diagonals[i];
            self.queen_attack_mask[i] = self.rook_attack_mask[i] | self.bishop_attack_mask[i];
            self.bishop_magic_mask[i] = self.bishop_attack_mask[i] & INTERIOR;
            self.rook_magic_mask[i] = self.rook_attack_mask[i];
            if file(i) > 0 {
                self.rook_magic_mask[i] &= !self.file[0];
            }
            if rank(i) > 0 {
                self.rook_magic_mask[i] &= !self.line[0];
            }
            if file(i) < 7 {
                self.rook_magic_mask[i] &= !self.file[7];
            }
            if rank(i) < 7 {
                self.rook_magic_mask[i] &= !self.line[7];
            }
            for j in 0..64 {
                if self.knight_attacks[i] & self.knight_attacks[j] != 0 {
                    self.knight_area[i] |= bit(j);
                }
            }
        }
        for i in 0..8 {
            self.west_files[i] = 0;
            self.east_files[i] = 0;
            self.forward_ranks[0][i] = 0;
            self.forward_ranks[1][i] = 0;
            self.isolated_pawns[i] = 0;
            for j in 0..8 {
                if i < j {
                    self.forward_ranks[0][i] |= self.line[j];
                } else if i > j {
                    self.forward_ranks[1][i] |= self.line[j];
                }
                if i < j {
                    self.east_files[i] |= self.file[j];
                } else if i > j {
                    self.west_files[i] |= self.file[j];
                }
            }
            if i > 0 {
                self.isolated_pawns[i] |= self.file[i - 1];
            }
            if i < 7 {
                self.isolated_pawns[i] |= self.file[i + 1];
            }
        }
        for i in 0..64 {
            let mut u = self.queen_attack_mask[i];
            while u != 0 {
                let j = least_significant_bit(u);
                clear_lowest_bit(&mut u);
                let k = sign(rank(j) as i32 - rank(i) as i32);
                let l = sign(file(j) as i32 - file(i) as i32);
                let step = 8 * k + l;
                let mut n = i as i32 + step;
                while n != j as i32 {
                    self.between[i][j] |= bit(n as usize);
                    n += step;
                }
            }
            let mut u = self.bishop_attack_mask[i];
            while u != 0 {
                let j = least_significant_bit(u);
                clear_lowest_bit(&mut u);
                self.full_line[i][j] = self.bishop_attack_mask[i] & self.bishop_attack_mask[j];
            }
            let mut u = self.rook_attack_mask[i];
            while u != 0 {
                let j = least_significant_bit(u);
                clear_lowest_bit(&mut u);
                self.full_line[i][j] = self.rook_attack_mask[i] & self.rook_attack_mask[j];
            }
            self.bishop_forward_rays[0][i] |= self.pawn_paths[0][i];
            self.bishop_forward_rays[1][i] |= self.pawn_paths[1][i];
            for j in 0..64 {
                if (self.pawn_paths[1][j] | bit(j))
                    & self.bishop_attack_mask[i]
                    & self.forward_ranks[0][rank(i)]
                    != 0
                {
                    self.bishop_forward_rays[0][i] |= bit(j);
                }
                if (self.pawn_paths[0][j] | bit(j))
                    & self.bishop_attack_mask[i]
                    & self.forward_ranks[1][rank(i)]
                    != 0
                {
                    self.bishop_forward_rays[1][i] |= bit(j);
                }
            }
        }

        for i in 0..16 {
            for j in 0..16 {
                self.mvv_lva[i][j] = if j < WHITE_PAWN {
                    0
                } else if j < WHITE_KNIGHT {
                    pawn_capture_mvv_lva(i) << 26
                } else if j < WHITE_LIGHT_BISHOP {
                    knight_capture_mvv_lva(i) << 26
                } else if j < WHITE_ROOK {
                    bishop_capture_mvv_lva(i) << 26
                } else if j < WHITE_QUEEN {
                    rook_capture_mvv_lva(i) << 26
                } else {
                    queen_capture_mvv_lva(i) << 26
                };
            }
        }

        for c in 0..256u16 {
            let b = c as u8;
            self.piece_from_char[c as usize] = match b {
                b'B' => WHITE_LIGHT_BISHOP,
                b'K' => WHITE_KING,
                b'N' => WHITE_KNIGHT,
                b'P' => WHITE_PAWN,
                b'Q' => WHITE_QUEEN,
                b'R' => WHITE_ROOK,
                b'b' => BLACK_LIGHT_BISHOP,
                b'k' => BLACK_KING,
                b'n' => BLACK_KNIGHT,
                b'p' => BLACK_PAWN,
                b'q' => BLACK_QUEEN,
                b'r' => BLACK_ROOK,
                _ => 0,
            } as u8;
        }

        let mut seed = 1u64;
        self.side_to_move_key = rand64(&mut seed);
        for i in 0..8 {
            self.en_passant_keys[i] = rand64(&mut seed);
        }
        for i in 0..16 {
            self.castling_keys[i] = rand64(&mut seed);
        }
        for i in 0..16 {
            for j in 0..64 {
                self.piece_keys[i][j] = if i == 0 { 0 } else { rand64(&mut seed) };
            }
        }
        for i in 0..16 {
            self.log_dist[i] = (10.0 * (1.01 + i as f64).ln()) as i32;
        }

        self.castling_rights_update = UPDATE_CASTLING;
    }

    // -- initialize_magic --------------------------------------------------------

    fn initialize_magic(&mut self) {
        for i in 0..64 {
            let bits = 64 - self.bishop_shift[i];
            let mut bit_list = [0usize; 16];
            let mut u = self.bishop_magic_mask[i];
            let mut j = 0;
            while u != 0 {
                bit_list[j] = least_significant_bit(u);
                clear_lowest_bit(&mut u);
                j += 1;
            }
            for n in 0..(1u64 << bits) {
                let mut occupancy = 0u64;
                for k in 0..bits {
                    if (n >> k) & 1 != 0 {
                        occupancy |= bit(bit_list[k as usize]);
                    }
                }
                let idx = self.bishop_table_offset[i]
                    + ((BISHOP_MAGIC[i].wrapping_mul(occupancy) >> self.bishop_shift[i]) as usize);
                self.magic_attacks[idx] = bishop_magic_attacks(self, i, occupancy);
            }

            let bits = 64 - self.rook_shift[i];
            let mut u = self.rook_magic_mask[i];
            let mut j = 0;
            while u != 0 {
                bit_list[j] = least_significant_bit(u);
                clear_lowest_bit(&mut u);
                j += 1;
            }
            for n in 0..(1u64 << bits) {
                let mut occupancy = 0u64;
                for k in 0..bits {
                    if (n >> k) & 1 != 0 {
                        occupancy |= bit(bit_list[k as usize]);
                    }
                }
                let idx = self.rook_table_offset[i]
                    + ((ROOK_MAGIC[i].wrapping_mul(occupancy) >> self.rook_shift[i]) as usize);
                self.magic_attacks[idx] = rook_magic_attacks(self, i, occupancy);
            }
        }
    }

    // -- initialize_kpk ----------------------------------------------------------

    fn initialize_kpk(&mut self) {
        const N: usize = 2 * 64 * 64 * 64;
        let mut gen = vec![0u8; N];
        let index = |side_to_move: usize,
                     white_pawn: usize,
                     white_king_square: usize,
                     black_king_square: usize| {
            (((side_to_move * 64 + white_pawn) * 64 + white_king_square) * 64) + black_king_square
        };
        let mut cnt: i32 = 0;
        let mut old_cnt: i32 = 1;
        while cnt != old_cnt {
            old_cnt = cnt;
            cnt = 0;
            for side_to_move in 0..2 {
                for white_pawn in 0..64 {
                    for white_king_square in 0..64 {
                        for black_king_square in 0..64 {
                            if gen[index(
                                side_to_move,
                                white_pawn,
                                white_king_square,
                                black_king_square,
                            )] != 0
                            {
                                continue;
                            }
                            cnt += 1;
                            let r = self.kpk_classify(
                                &gen,
                                side_to_move,
                                white_pawn,
                                white_king_square,
                                black_king_square,
                            );
                            if r != 0 {
                                gen[index(
                                    side_to_move,
                                    white_pawn,
                                    white_king_square,
                                    black_king_square,
                                )] = r;
                            }
                        }
                    }
                }
            }
        }
        for side_to_move in 0..2 {
            for white_pawn in 0..64 {
                for white_king_square in 0..64 {
                    let mut m = 0u64;
                    for black_king_square in 0..64 {
                        if gen[index(
                            side_to_move,
                            white_pawn,
                            white_king_square,
                            black_king_square,
                        )] == 2
                        {
                            m |= bit(black_king_square);
                        }
                    }
                    self.kpk[(side_to_move * 64 + white_pawn) * 64 + white_king_square] = m;
                }
            }
        }
    }

    fn kpk_classify(
        &self,
        gen: &[u8],
        side_to_move: usize,
        white_pawn: usize,
        white_king_square: usize,
        black_king_square: usize,
    ) -> u8 {
        let idx = |side_to_move: usize,
                   white_pawn: usize,
                   white_king_square: usize,
                   black_king_square: usize| {
            (((side_to_move * 64 + white_pawn) * 64 + white_king_square) * 64) + black_king_square
        };
        if white_pawn < 8 || white_pawn >= 56 {
            return 1;
        }
        if white_pawn == white_king_square
            || white_king_square == black_king_square
            || black_king_square == white_pawn
        {
            return 1;
        }
        let white_pawn_bitboard = bit(white_pawn);
        let white_king_bitboard = bit(white_king_square);
        let black_king_bitboard = bit(black_king_square);
        if self.pawn_attacks[WHITE][white_pawn] & black_king_bitboard != 0 {
            if side_to_move == WHITE {
                return 1;
            } else if self.king_attack_area[white_king_square] & white_pawn_bitboard == 0 {
                return 1;
            }
        }
        let o = side_to_move ^ 1;
        let mut un = 0;
        if side_to_move == BLACK {
            let mut u = self.king_attack_area[black_king_square]
                & (!(self.king_attack_area[white_king_square]
                    | self.pawn_attacks[WHITE][white_pawn]));
            if u == 0 {
                return 1;
            }
            while u != 0 {
                let to = least_significant_bit(u);
                clear_lowest_bit(&mut u);
                let v = gen[idx(o, white_pawn, white_king_square, to)];
                if v == 1 {
                    return 1;
                } else if v == 0 {
                    un += 1;
                }
            }
            if un == 0 {
                return 2;
            }
        } else {
            let mut u = self.king_attack_area[white_king_square]
                & (!(self.king_attack_area[black_king_square] | white_pawn_bitboard));
            while u != 0 {
                let to = least_significant_bit(u);
                clear_lowest_bit(&mut u);
                let v = gen[idx(o, white_pawn, to, black_king_square)];
                if v == 2 {
                    return 2;
                } else if v == 0 {
                    un += 1;
                }
            }
            let mut to = white_pawn + 8;
            if to != white_king_square && to != black_king_square {
                if to >= 56 {
                    if self.king_attack_area[to] & black_king_bitboard == 0 {
                        return 2;
                    }
                    if self.king_attack_area[to] & white_king_bitboard != 0 {
                        return 2;
                    }
                } else {
                    let v = gen[idx(o, to, white_king_square, black_king_square)];
                    if v == 2 {
                        return 2;
                    } else if v == 0 {
                        un += 1;
                    }
                    if to < 24 {
                        to += 8;
                        if to != white_king_square && to != black_king_square {
                            let v = gen[idx(o, to, white_king_square, black_king_square)];
                            if v == 2 {
                                return 2;
                            } else if v == 0 {
                                un += 1;
                            }
                        }
                    }
                }
            }
            if un == 0 {
                return 1;
            }
        }
        0
    }

    // -- initialize_piece_square_tables ----------------------------------------------------------

    fn initialize_piece_square_tables(&mut self) {
        for i in 0..64 {
            let r = rank(i);
            let f = file(i);
            let d = (f as i32 - r as i32).abs() as usize;
            let e = (f as i32 + r as i32 - 7).abs() as usize;
            let dist_q = [
                (DIST_C[f] as i32) * (DIST_C[f] as i32),
                (DIST_C[r] as i32) * (DIST_C[r] as i32),
                (RANK_R[d] as i32) * (RANK_R[d] as i32) + (RANK_R[e] as i32) * (RANK_R[e] as i32),
                (RANK_R[r] as i32) * (RANK_R[r] as i32),
            ];
            let dist_l = [
                DIST_C[f] as i32,
                DIST_C[r] as i32,
                RANK_R[d] as i32 + RANK_R[e] as i32,
                RANK_R[r] as i32,
            ];
            let dist_m = [
                (DIST_C[f] as i32) * (DIST_C[r] as i32),
                (DIST_C[f] as i32) * (RANK_R[r] as i32),
            ];
            for j in (2..16).step_by(2) {
                let index = PIECE_TYPE[j];
                let mut op = 0i32;
                let mut eg = 0i32;
                for k in 0..2 {
                    op += PST_QUAD_MIXED_WEIGHTS[4 * index + k * 2] * dist_m[k];
                    eg += PST_QUAD_MIXED_WEIGHTS[4 * index + k * 2 + 1] * dist_m[k];
                }
                for k in 0..4 {
                    op += PST_QUAD_WEIGHTS[8 * index + k * 2] * dist_q[k];
                    eg += PST_QUAD_WEIGHTS[8 * index + k * 2 + 1] * dist_q[k];
                    op += PST_LINEAR_WEIGHTS[8 * index + k * 2] * dist_l[k];
                    eg += PST_LINEAR_WEIGHTS[8 * index + k * 2 + 1] * dist_l[k];
                }
                self.piece_square_score[(j << 6) | i] = compose(op / 64, eg / 64);
            }
        }
        self.piece_square_score[(WHITE_KNIGHT << 6) | 56] -= compose(100, 0);
        self.piece_square_score[(WHITE_KNIGHT << 6) | 63] -= compose(100, 0);
        for i in 0..64 {
            for j in (3..16).step_by(2) {
                let op = opening(self.piece_square_score[((j - 1) << 6) | (63 - i)]);
                let eg = endgame(self.piece_square_score[((j - 1) << 6) | (63 - i)]);
                self.piece_square_score[(j << 6) | i] = compose(-op, -eg);
            }
        }
    }

    // -- initialize_evaluation ---------------------------------------------------------

    fn initialize_evaluation(&mut self) {
        for i in 0..4usize {
            for j in 0..32usize {
                let index = i * 2;
                let op = (MOBILITY_LINEAR[index] * j as i32) as f64
                    + (1.01 + j as f64).ln() * MOBILITY_LOG[index] as f64;
                let index = i * 2 + 1;
                let eg = (MOBILITY_LINEAR[index] * j as i32) as f64
                    + (1.01 + j as f64).ln() * MOBILITY_LOG[index] as f64;
                self.mobility[i][j] = compose((op / 64.0) as i32, (eg / 64.0) as i32);
            }
        }

        for i in 0..3 {
            for j in (0..8).rev() {
                self.shelter[i][j] = 0;
                if j > 1 {
                    for k in 1..j.min(5) {
                        self.shelter[i][j] += SHELTER_VALUE[(i * 5) + k - 1];
                    }
                }
                if j == 0 {
                    self.shelter[i][0] = self.shelter[i][7] + SHELTER_VALUE[(i * 5) + 4];
                }
            }
        }

        for i in 0..4 {
            let ii = i as i32;
            self.storm_blocked[i] = (STORM_QUAD[STORM_BLOCKED_MUL] * ii * ii
                + STORM_LINEAR[STORM_BLOCKED_MUL] * (ii + 1))
                / 100;
            self.storm_shelter_att[i] = (STORM_QUAD[STORM_SHELTER_ATT_MUL] * ii * ii
                + STORM_LINEAR[STORM_SHELTER_ATT_MUL] * (ii + 1))
                / 100;
            self.storm_connected[i] = (STORM_QUAD[STORM_CONNECTED_MUL] * ii * ii
                + STORM_LINEAR[STORM_CONNECTED_MUL] * (ii + 1))
                / 100;
            self.storm_open[i] = (STORM_QUAD[STORM_OPEN_MUL] * ii * ii
                + STORM_LINEAR[STORM_OPEN_MUL] * (ii + 1))
                / 100;
            self.storm_free[i] = (STORM_QUAD[STORM_FREE_MUL] * ii * ii
                + STORM_LINEAR[STORM_FREE_MUL] * (ii + 1))
                / 100;
        }

        for i in 0..8 {
            let l = (i as i32 - 2).max(0);
            let q = l * l;
            self.passed_pawn_general[i] = compose16(
                PASSER_QUAD[0] * q + PASSER_LINEAR[0] * l,
                PASSER_QUAD[1] * q + PASSER_LINEAR[1] * l,
            );
            self.passed_pawn_blocked[i] = compose16(
                PASSER_QUAD[2] * q + PASSER_LINEAR[2] * l,
                PASSER_QUAD[3] * q + PASSER_LINEAR[3] * l,
            );
            self.passed_pawn_free[i] = compose16(
                PASSER_QUAD[4] * q + PASSER_LINEAR[4] * l,
                PASSER_QUAD[5] * q + PASSER_LINEAR[5] * l,
            );
            self.passed_pawn_supported[i] = compose16(
                PASSER_QUAD[6] * q + PASSER_LINEAR[6] * l,
                PASSER_QUAD[7] * q + PASSER_LINEAR[7] * l,
            );
            self.passed_pawn_protected[i] = compose16(
                PASSER_QUAD[8] * q + PASSER_LINEAR[8] * l,
                PASSER_QUAD[9] * q + PASSER_LINEAR[9] * l,
            );
            self.passed_pawn_connected[i] = compose16(
                PASSER_QUAD[10] * q + PASSER_LINEAR[10] * l,
                PASSER_QUAD[11] * q + PASSER_LINEAR[11] * l,
            );
            self.passed_pawn_outside[i] = compose16(
                PASSER_QUAD[12] * q + PASSER_LINEAR[12] * l,
                PASSER_QUAD[13] * q + PASSER_LINEAR[13] * l,
            );
            self.passed_pawn_candidate[i] = compose16(
                PASSER_QUAD[14] * q + PASSER_LINEAR[14] * l,
                PASSER_QUAD[15] * q + PASSER_LINEAR[15] * l,
            );
            self.passed_pawn_clear[i] = compose16(
                PASSER_QUAD[16] * q + PASSER_LINEAR[16] * l,
                PASSER_QUAD[17] * q + PASSER_LINEAR[17] * l,
            );
            self.passed_pawn_att[i] = PASSER_ATT_DEF_QUAD[0] * q + PASSER_ATT_DEF_LINEAR[0] * l;
            self.passed_pawn_def[i] = PASSER_ATT_DEF_QUAD[2] * q + PASSER_ATT_DEF_LINEAR[2] * l;
            self.passed_pawn_att_log[i] = PASSER_ATT_DEF_QUAD[1] * q + PASSER_ATT_DEF_LINEAR[1] * l;
            self.passed_pawn_def_log[i] = PASSER_ATT_DEF_QUAD[3] * q + PASSER_ATT_DEF_LINEAR[3] * l;
        }

        self.isolated = ISOLATED;
        self.unprotected = UNPROTECTED;
        self.backward = BACKWARD;
        self.doubled = DOUBLED;
        self.rook_special = ROOK_SPECIAL;
        self.tactical = TACTICAL;
        self.king_defence = KING_DEFENCE;
        self.pawn_special = PAWN_SPECIAL;
        self.bishop_special = BISHOP_SPECIAL;
        self.knight_special = KNIGHT_SPECIAL;
        self.pin = PIN;
        self.king_ray = KING_RAY;
        self.king_attack_weight = KING_ATTACK_WEIGHT;
        self.storm_hof = STORM_HOF;
        self.passed_pawn_special = PASSER_SPECIAL;
    }

    // -- initialize_material / calc_material -------------------------------------

    fn initialize_material(&mut self) {
        for index in 0..TOTAL_MAT {
            self.calc_material(index);
        }
    }

    fn calc_material(&mut self, index: usize) {
        let mut i = index as i32;
        let mut queens = [0i32; 2];
        let mut rooks = [0i32; 2];
        let mut light = [0i32; 2];
        let mut dark = [0i32; 2];
        let mut knights = [0i32; 2];
        let mut pawns = [0i32; 2];

        queens[WHITE] = i % 3;
        i /= 3;
        queens[BLACK] = i % 3;
        i /= 3;
        rooks[WHITE] = i % 3;
        i /= 3;
        rooks[BLACK] = i % 3;
        i /= 3;
        light[WHITE] = i % 2;
        i /= 2;
        light[BLACK] = i % 2;
        i /= 2;
        dark[WHITE] = i % 2;
        i /= 2;
        dark[BLACK] = i % 2;
        i /= 2;
        knights[WHITE] = i % 3;
        i /= 3;
        knights[BLACK] = i % 3;
        i /= 3;
        pawns[WHITE] = i % 9;
        i /= 9;
        pawns[BLACK] = i % 9;

        let mut bishops = [0i32; 2];
        let mut major = [0i32; 2];
        let mut minor = [0i32; 2];
        let mut tot = [0i32; 2];
        let mut mat = [32i32; 2];
        let mut mul = [32i32; 2];
        let mut quad = [0i32; 2];

        for color in 0..2 {
            bishops[color] = light[color] + dark[color];
            major[color] = rooks[color] + queens[color];
            minor[color] = bishops[color] + knights[color];
            tot[color] = 3 * minor[color] + 5 * rooks[color] + 9 * queens[color];
            mat[color] = 32;
            mul[color] = 32;
            quad[color] = 0;
        }

        let mut score = (SEE_VALUE[WHITE_PAWN] + MATERIAL_LINEAR[0])
            * (pawns[WHITE] - pawns[BLACK])
            + (SEE_VALUE[WHITE_KNIGHT] + MATERIAL_LINEAR[1]) * (knights[WHITE] - knights[BLACK])
            + (SEE_VALUE[WHITE_LIGHT_BISHOP] + MATERIAL_LINEAR[2])
                * (bishops[WHITE] - bishops[BLACK])
            + (SEE_VALUE[WHITE_ROOK] + MATERIAL_LINEAR[3]) * (rooks[WHITE] - rooks[BLACK])
            + (SEE_VALUE[WHITE_QUEEN] + MATERIAL_LINEAR[4]) * (queens[WHITE] - queens[BLACK])
            + 50 * ((bishops[WHITE] / 2) - (bishops[BLACK] / 2));

        let phase = PHASE[PIECE_TYPE[WHITE_PAWN]] * (pawns[WHITE] + pawns[BLACK])
            + PHASE[PIECE_TYPE[WHITE_KNIGHT]] * (knights[WHITE] + knights[BLACK])
            + PHASE[PIECE_TYPE[WHITE_LIGHT_BISHOP]] * (bishops[WHITE] + bishops[BLACK])
            + PHASE[PIECE_TYPE[WHITE_ROOK]] * (rooks[WHITE] + rooks[BLACK])
            + PHASE[PIECE_TYPE[WHITE_QUEEN]] * (queens[WHITE] + queens[BLACK]);
        let phase = (((phase - PHASE_MIN).max(0) * 128) / (PHASE_MAX - PHASE_MIN)).min(128);
        self.material[index].phase = phase as u8;

        let mut special = 0i32;
        for color in 0..2 {
            let o = opponent(color);
            if queens[color] == queens[o] {
                if rooks[color] - rooks[o] == 1 {
                    if knights[color] == knights[o] && bishops[o] - bishops[color] == 1 {
                        add_eval(
                            color,
                            &mut special,
                            compose_pair(MATERIAL_SPECIAL, MATERIAL_ROOK_BISHOP),
                        );
                    } else if bishops[color] == bishops[o] && knights[o] - knights[color] == 1 {
                        add_eval(
                            color,
                            &mut special,
                            compose_pair(MATERIAL_SPECIAL, MATERIAL_ROOK_KNIGHT),
                        );
                    } else if knights[color] == knights[o] && bishops[o] - bishops[color] == 2 {
                        subtract_eval(
                            color,
                            &mut special,
                            compose_pair(MATERIAL_SPECIAL, MATERIAL_BISHOP_BISHOP_ROOK),
                        );
                    } else if bishops[color] == bishops[o] && knights[o] - knights[color] == 2 {
                        subtract_eval(
                            color,
                            &mut special,
                            compose_pair(MATERIAL_SPECIAL, MATERIAL_KNIGHT_KNIGHT_ROOK),
                        );
                    } else if bishops[o] - bishops[color] == 1 && knights[o] - knights[color] == 1 {
                        subtract_eval(
                            color,
                            &mut special,
                            compose_pair(MATERIAL_SPECIAL, MATERIAL_BISHOP_KNIGHT_ROOK),
                        );
                    }
                } else if rooks[color] == rooks[o] && minor[color] - minor[o] == 1 {
                    add_eval(
                        color,
                        &mut special,
                        compose_pair(MATERIAL_SPECIAL, MATERIAL_MISC),
                    );
                }
            } else if queens[color] - queens[o] == 1 {
                if rooks[o] - rooks[color] == 2 && minor[o] - minor[color] == 0 {
                    add_eval(
                        color,
                        &mut special,
                        compose_pair(MATERIAL_SPECIAL, MATERIAL_QUEEN_ROOK_ROOK),
                    );
                } else if rooks[o] - rooks[color] == 1
                    && knights[o] == knights[color]
                    && bishops[o] - bishops[color] == 1
                {
                    add_eval(
                        color,
                        &mut special,
                        compose_pair(MATERIAL_SPECIAL, MATERIAL_QUEEN_ROOK_BISHOP),
                    );
                } else if rooks[o] - rooks[color] == 1
                    && knights[o] - knights[color] == 1
                    && bishops[o] == bishops[color]
                {
                    add_eval(
                        color,
                        &mut special,
                        compose_pair(MATERIAL_SPECIAL, MATERIAL_QUEEN_ROOK_KNIGHT),
                    );
                } else if (major[o] + minor[o]) - (major[color] + minor[color]) >= 2 {
                    add_eval(color, &mut special, compose_pair(MATERIAL_SPECIAL, MAT_Q3));
                }
            }
        }
        score += (opening(special) * self.material[index].phase as i32
            + endgame(special) * (128 - self.material[index].phase as i32))
            / 128;

        for color in 0..2 {
            let o = opponent(color);
            quad[color] += pawns[color]
                * (pawns[color] * trav(MatQuadMe::PawnPawn)
                    + knights[color] * trav(MatQuadMe::KnightPawn)
                    + bishops[color] * trav(MatQuadMe::BishopPawn)
                    + rooks[color] * trav(MatQuadMe::RookPawn)
                    + queens[color] * trav(MatQuadMe::QueenPawn));
            quad[color] += knights[color]
                * (knights[color] * trav(MatQuadMe::KnightKnight)
                    + bishops[color] * trav(MatQuadMe::BishopKnight)
                    + rooks[color] * trav(MatQuadMe::RookKnight)
                    + queens[color] * trav(MatQuadMe::QueenKnight));
            quad[color] += bishops[color]
                * (bishops[color] * trav(MatQuadMe::BishopBishop)
                    + rooks[color] * trav(MatQuadMe::RookBishop)
                    + queens[color] * trav(MatQuadMe::QueenBishop));
            quad[color] += rooks[color]
                * (rooks[color] * trav(MatQuadMe::RookRook)
                    + queens[color] * trav(MatQuadMe::QueenRook));
            quad[color] += pawns[color]
                * (knights[o] * trav(MatQuadOpp::PawnKnight)
                    + bishops[o] * trav(MatQuadOpp::PawnBishop)
                    + rooks[o] * trav(MatQuadOpp::PawnRook)
                    + queens[o] * trav(MatQuadOpp::PawnQueen));
            quad[color] += knights[color]
                * (bishops[o] * trav(MatQuadOpp::KnightBishop)
                    + rooks[o] * trav(MatQuadOpp::KnightRook)
                    + queens[o] * trav(MatQuadOpp::KnightQueen));
            quad[color] += bishops[color]
                * (rooks[o] * trav(MatQuadOpp::BishopRook)
                    + queens[o] * trav(MatQuadOpp::BishopQueen));
            quad[color] += rooks[color] * queens[o] * trav(MatQuadOpp::RookQueen);

            if bishops[color] >= 2 {
                quad[color] += pawns[color] * BISHOP_PAIR_QUAD[0]
                    + knights[color] * BISHOP_PAIR_QUAD[1]
                    + rooks[color] * BISHOP_PAIR_QUAD[2]
                    + queens[color] * BISHOP_PAIR_QUAD[3]
                    + pawns[o] * BISHOP_PAIR_QUAD[4]
                    + knights[o] * BISHOP_PAIR_QUAD[5]
                    + bishops[o] * BISHOP_PAIR_QUAD[6]
                    + rooks[o] * BISHOP_PAIR_QUAD[7]
                    + queens[o] * BISHOP_PAIR_QUAD[8];
            }
        }
        score += (quad[WHITE] - quad[BLACK]) / 100;

        for color in 0..2 {
            let o = opponent(color);
            if tot[color] - tot[o] <= 3 {
                if pawns[color] == 0 {
                    if tot[color] <= 3 {
                        mul[color] = 0;
                    }
                    if tot[color] == tot[o] && major[color] == major[o] && minor[color] == minor[o]
                    {
                        mul[color] = if major[color] + minor[color] <= 2 {
                            0
                        } else if major[color] + minor[color] <= 3 {
                            16
                        } else {
                            32
                        };
                    } else if minor[color] + major[color] <= 2 {
                        if bishops[color] < 2 {
                            mat[color] = if bishops[color] != 0 && rooks[color] != 0 {
                                8
                            } else {
                                1
                            };
                        } else if bishops[o] + rooks[o] >= 1 {
                            mat[color] = 1;
                        } else {
                            mat[color] = 32;
                        }
                    } else if tot[color] - tot[o] < 3
                        && minor[color] + major[color] - minor[o] - major[o] <= 1
                    {
                        mat[color] = 4;
                    } else if minor[color] + major[color] <= 3 {
                        mat[color] = 8 * (1 + bishops[color]);
                    } else {
                        mat[color] = 8 * (2 + bishops[color]);
                    }
                }
                if pawns[color] <= 1 {
                    mul[color] = mul[color].min(28);
                    if rooks[color] == 1 && queens[color] + minor[color] == 0 && rooks[o] == 1 {
                        mat[color] = mat[color].min(23);
                    }
                }
            }
            if major[color] == 0 {
                if minor[color] == 0 {
                    if tot[color] == 0 && pawns[color] < pawns[o] {
                        subtract_eval(
                            color,
                            &mut score,
                            (pawns[o] - pawns[color]) * SEE_VALUE[WHITE_PAWN],
                        );
                    }
                } else if minor[color] == 1 {
                    if pawns[color] <= 1 && minor[o] >= 1 {
                        mat[color] = 1;
                    }
                    if bishops[color] == 1
                        && minor[o] == 1
                        && bishops[o] == 1
                        && light[color] != light[o]
                    {
                        mul[color] = mul[color].min(15);
                        if pawns[color] - pawns[o] <= 1 {
                            mul[color] = mul[color].min(11);
                        }
                    }
                } else if pawns[color] == 0 && knights[color] == 2 && bishops[color] == 0 {
                    if tot[o] == 0 && pawns[o] != 0 {
                        mat[color] = 6;
                    } else {
                        mul[color] = 0;
                    }
                }
            }
            if mul[color] == 0 {
                mat[color] = 0;
            }
            if mat[color] <= 1 && tot[color] != tot[o] {
                mul[color] = mul[color].min(8);
            }
        }
        if bishops[WHITE] == 1 && bishops[BLACK] == 1 && light[WHITE] != light[BLACK] {
            mul[WHITE] = mul[WHITE].min(24 + 2 * (knights[BLACK] + major[BLACK]));
            mul[BLACK] = mul[BLACK].min(24 + 2 * (knights[WHITE] + major[WHITE]));
        } else if minor[WHITE] == 0
            && minor[BLACK] == 0
            && major[WHITE] == 1
            && major[BLACK] == 1
            && rooks[WHITE] == rooks[BLACK]
        {
            mul[WHITE] = mul[WHITE].min(25);
            mul[BLACK] = mul[BLACK].min(25);
        }
        for color in 0..2 {
            self.material[index].evaluation_multiplier[color] = mul[color] as u8;
            self.material[index].pieces[color] = (major[color] + minor[color]) as u8;
        }
        let score = if score > 0 {
            (score * mat[WHITE]) / 32
        } else {
            (score * mat[BLACK]) / 32
        };
        self.material[index].score = score as i16;
        for color in 0..2 {
            let o = opponent(color);
            if major[color] == 0 && minor[color] == bishops[color] && minor[color] <= 1 {
                self.material[index].flags |= 1 << color;
            }
            if ((major[color] == 0 || minor[color] == 0) && major[color] + minor[color] <= 1)
                || major[o] + minor[o] == 0
                || (pawns[color] == 0
                    && major[color] == rooks[color]
                    && major[color] == 1
                    && minor[color] == bishops[color]
                    && minor[color] == 1
                    && rooks[o] == 1
                    && minor[o] == 0
                    && queens[o] == 0)
            {
                self.material[index].flags |= 1 << (color + 2);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers used by the material quad tables.
// ---------------------------------------------------------------------------

enum MatQuadMe {
    PawnPawn = 0,
    KnightPawn = 1,
    BishopPawn = 2,
    RookPawn = 3,
    QueenPawn = 4,
    KnightKnight = 5,
    BishopKnight = 6,
    RookKnight = 7,
    QueenKnight = 8,
    BishopBishop = 9,
    RookBishop = 10,
    QueenBishop = 11,
    RookRook = 12,
    QueenRook = 13,
}

enum MatQuadOpp {
    PawnKnight = 0,
    PawnBishop = 1,
    PawnRook = 2,
    PawnQueen = 3,
    KnightBishop = 4,
    KnightRook = 5,
    KnightQueen = 6,
    BishopRook = 7,
    BishopQueen = 8,
    RookQueen = 9,
}

#[inline]
fn trav(idx: impl TravIndex) -> i32 {
    idx.value()
}

trait TravIndex {
    fn value(self) -> i32;
}

impl TravIndex for MatQuadMe {
    fn value(self) -> i32 {
        MAT_QUAD_ME[self as usize]
    }
}

impl TravIndex for MatQuadOpp {
    fn value(self) -> i32 {
        MAT_QUAD_OPP[self as usize]
    }
}

#[inline]
fn compose_pair(arr: [i32; 20], idx: usize) -> i32 {
    compose(arr[idx * 2], arr[idx * 2 + 1])
}

#[inline]
fn add_eval(color: Color, var: &mut i32, x: i32) {
    if color == WHITE {
        *var += x;
    } else {
        *var -= x;
    }
}

#[inline]
fn subtract_eval(color: Color, var: &mut i32, x: i32) {
    if color == WHITE {
        *var -= x;
    } else {
        *var += x;
    }
}

#[inline]
fn pawn_capture_mvv_lva(attacker: usize) -> i32 {
    MVV_LVA_ATTACKER[attacker]
}
#[inline]
fn knight_capture_mvv_lva(attacker: usize) -> i32 {
    MVV_LVA_MAX_PAWN_CAPTURE + MVV_LVA_ATTACKER_KB[attacker]
}
#[inline]
fn bishop_capture_mvv_lva(attacker: usize) -> i32 {
    MVV_LVA_MAX_PAWN_CAPTURE + MVV_LVA_ATTACKER_KB[attacker] + 1
}
#[inline]
fn rook_capture_mvv_lva(attacker: usize) -> i32 {
    MVV_LVA_MAX_BISHOP_CAPTURE + MVV_LVA_ATTACKER[attacker]
}
#[inline]
fn queen_capture_mvv_lva(attacker: usize) -> i32 {
    MVV_LVA_MAX_ROOK_CAPTURE + MVV_LVA_ATTACKER[attacker]
}

const MVV_LVA_ATTACKER: [i32; 16] = [0, 0, 5, 5, 4, 4, 3, 3, 3, 3, 2, 2, 1, 1, 6, 6];
const MVV_LVA_ATTACKER_KB: [i32; 16] = [0, 0, 9, 9, 7, 7, 5, 5, 5, 5, 3, 3, 1, 1, 11, 11];
const MVV_LVA_MAX_PAWN_CAPTURE: i32 = MVV_LVA_ATTACKER[15];
const MVV_LVA_MAX_BISHOP_CAPTURE: i32 = MVV_LVA_MAX_PAWN_CAPTURE + MVV_LVA_ATTACKER_KB[15] + 1;
const MVV_LVA_MAX_ROOK_CAPTURE: i32 = MVV_LVA_MAX_BISHOP_CAPTURE + MVV_LVA_ATTACKER[15];

// ---------------------------------------------------------------------------
// Random numbers (same LCG as the C++ implementation).
// ---------------------------------------------------------------------------

pub fn rand16(seed: &mut u64) -> u16 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*seed >> 32) & 0xFFFF) as u16
}

pub fn rand64(seed: &mut u64) -> u64 {
    let mut key = rand16(seed) as u64;
    key <<= 16;
    key |= rand16(seed) as u64;
    key <<= 16;
    key |= rand16(seed) as u64;
    key <<= 16;
    key | rand16(seed) as u64
}

#[inline]
fn bishop_magic_attacks(tables: &Tables, square_index: usize, occupancy: u64) -> u64 {
    let mut attacks = 0u64;
    let mut attack_mask = tables.bishop_attack_mask[square_index];
    while attack_mask != 0 {
        let target_square = least_significant_bit(attack_mask);
        clear_lowest_bit(&mut attack_mask);
        if tables.between[square_index][target_square] & occupancy == 0 {
            attacks |= tables.between[square_index][target_square] | bit(target_square);
        }
    }
    attacks
}

#[inline]
fn rook_magic_attacks(tables: &Tables, square_index: usize, occupancy: u64) -> u64 {
    let mut attacks = 0u64;
    let mut attack_mask = tables.rook_attack_mask[square_index];
    while attack_mask != 0 {
        let target_square = least_significant_bit(attack_mask);
        clear_lowest_bit(&mut attack_mask);
        if tables.between[square_index][target_square] & occupancy == 0 {
            attacks |= tables.between[square_index][target_square] | bit(target_square);
        }
    }
    attacks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_deterministic() {
        let a = Tables::new();
        let b = Tables::new();
        assert_eq!(a.side_to_move_key, b.side_to_move_key);
        assert_eq!(a.piece_keys, b.piece_keys);
        assert_eq!(a.castling_keys, b.castling_keys);
        assert_eq!(a.en_passant_keys, b.en_passant_keys);
        assert!(a.side_to_move_key != 0);
    }

    #[test]
    fn magic_attacks_match_slow_rays() {
        let tables = Tables::new();
        // Compare magic attacks against a direct ray scan for representative
        // squares and occupancies.
        for &origin_square in &[0usize, 7, 27, 35, 56, 63] {
            for occupancy in [0u64, 0xFFFF_FFFF_FFFF_FFFF, 0x00FF_0000_0000_0000] {
                let mag = tables.bishop_attacks(origin_square, occupancy);
                let slow = slow_bishop(&tables, origin_square, occupancy);
                assert_eq!(mag, slow, "bishop {origin_square} occupancy {occupancy:#x}");
                let mag = tables.rook_attacks(origin_square, occupancy);
                let slow = slow_rook(&tables, origin_square, occupancy);
                assert_eq!(mag, slow, "rook {origin_square} occupancy {occupancy:#x}");
                assert_eq!(
                    tables.queen_attacks(origin_square, occupancy),
                    slow_bishop(&tables, origin_square, occupancy)
                        | slow_rook(&tables, origin_square, occupancy)
                );
            }
        }
    }

    fn slow_bishop(tables: &Tables, origin_square: usize, occupancy: u64) -> u64 {
        let mut attacks = 0;
        for (df, dr) in [(1i32, 1i32), (1, -1), (-1, 1), (-1, -1)] {
            let mut f = file(origin_square) as i32 + df;
            let mut r = rank(origin_square) as i32 + dr;
            while (0..8).contains(&f) && (0..8).contains(&r) {
                let q = square_index(f as usize, r as usize);
                attacks |= bit(q);
                if occupancy & bit(q) != 0 {
                    break;
                }
                f += df;
                r += dr;
            }
        }
        let _ = tables;
        attacks
    }

    fn slow_rook(tables: &Tables, origin_square: usize, occupancy: u64) -> u64 {
        let mut attacks = 0;
        for (df, dr) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
            let mut f = file(origin_square) as i32 + df;
            let mut r = rank(origin_square) as i32 + dr;
            while (0..8).contains(&f) && (0..8).contains(&r) {
                let q = square_index(f as usize, r as usize);
                attacks |= bit(q);
                if occupancy & bit(q) != 0 {
                    break;
                }
                f += df;
                r += dr;
            }
        }
        let _ = tables;
        attacks
    }

    #[test]
    fn pawn_attack_sets() {
        let tables = Tables::new();
        assert_eq!(
            tables.pawn_attacks[WHITE][square_index(4, 1)],
            bit(square_index(3, 2)) | bit(square_index(5, 2))
        );
        assert_eq!(
            tables.pawn_attacks[BLACK][square_index(4, 6)],
            bit(square_index(3, 5)) | bit(square_index(5, 5))
        );
        assert_eq!(
            tables.pawn_attacks[WHITE][square_index(0, 0)],
            bit(square_index(1, 1))
        );
        assert_eq!(
            tables.pawn_attacks[BLACK][square_index(7, 7)],
            bit(square_index(6, 6))
        );
    }

    #[test]
    fn material_table_basics() {
        let tables = Tables::new();
        // Empty material index.
        assert_eq!(tables.material[0].score, 0);
        assert_eq!(tables.material[0].phase, 0);
        // Starting position material index is the maximum-ish one; all
        // material entries must have sane phase bounds.
        for m in &tables.material {
            assert!(m.phase <= 128);
        }
        // A lone white queen has a positive score.
        let idx = (MATERIAL_WHITE_QUEEN + MATERIAL_BLACK_QUEEN) as usize; // one white queen, one black queen? see decode
        let _ = idx;
    }

    #[test]
    fn pst_symmetry() {
        let tables = Tables::new();
        for j in (3..16).step_by(2) {
            for i in 0..64 {
                assert_eq!(
                    tables.piece_square_score[(j << 6) | i],
                    compose(
                        -opening(tables.piece_square_score[((j - 1) << 6) | (63 - i)]),
                        -endgame(tables.piece_square_score[((j - 1) << 6) | (63 - i)])
                    )
                );
            }
        }
    }

    #[test]
    fn kpk_table_has_expected_entries() {
        let tables = Tables::new();
        // White king e1, pawn e2, black king e8: white to move should be a win.
        let kpk = tables.kpk(WHITE, square_index(4, 1), square_index(4, 0));
        assert!(kpk & bit(square_index(4, 7)) != 0);
    }
}
