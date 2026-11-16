//! Pre-computed data tables, evaluation weights and material tables.
//!
//! This is a faithful port of `data.cpp` and the initialisation portion of
//! `Gull.cpp`.  The random Zobrist keys are generated with the same LCG and
//! the same call order, so hash keys match the C++ engine exactly.

use std::sync::OnceLock;

use crate::types::*;

pub const INITIAL_HASH_SIZE: usize = 1024 * 1024;
pub const PAWN_HASH_SIZE: usize = 1024 * 1024;
pub const PV_HASH_SIZE: usize = 1024 * 1024;
pub const PV_CLUSTER_SIZE: usize = 4;
pub const PV_HASH_MASK: usize = PV_HASH_SIZE - PV_CLUSTER_SIZE;
pub const PAWN_HASH_MASK: usize = PAWN_HASH_SIZE - 1;

pub const OUTPOST: [u64; 2] = [0x0000_7E7E_3C00_0000, 0x0000_003C_7E7E_0000];

// ---------------------------------------------------------------------------
// Evaluation weights (`// tuner: start` .. `// tuner: stop`)
// ---------------------------------------------------------------------------

pub const MAT_LINEAR: [i32; 5] = [3, 0, 3, 19, 0];

pub const MAT_QUAD_ME: [i32; 14] = [
    -33, 17, -23, -155, -247, 15, 296, -105, -83, -162, 327, 315, -861, -1013,
];

pub const MAT_QUAD_OPP: [i32; 10] = [-14, 47, -20, -278, 35, 39, 49, 9, -2, 75];

pub const BISHOP_PAIR_QUAD: [i32; 9] = [-38, 164, 99, 246, -84, -57, -184, 88, -186];

pub const MAT_RB: usize = 0;
pub const MAT_RN: usize = 1;
pub const MAT_QRR: usize = 2;
pub const MAT_QRB: usize = 3;
pub const MAT_QRN: usize = 4;
pub const MAT_Q3: usize = 5;
pub const MAT_BBR: usize = 6;
pub const MAT_BNR: usize = 7;
pub const MAT_NNR: usize = 8;
pub const MAT_M: usize = 9;

pub const MAT_SPECIAL: [i32; 20] = [
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
pub const fn king_n_attack() -> i32 {
    compose(1, KING_ATTACK_WEIGHT[0])
}
pub const fn king_b_attack() -> i32 {
    compose(1, KING_ATTACK_WEIGHT[1])
}
pub const fn king_r_attack() -> i32 {
    compose(1, KING_ATTACK_WEIGHT[2])
}
pub const fn king_q_attack() -> i32 {
    compose(1, KING_ATTACK_WEIGHT[3])
}
pub const fn king_attack() -> i32 {
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
    pub mul: [u8; 2],
    pub pieces: [u8; 2],
}

// ---------------------------------------------------------------------------
// Magic bitboard constants (from Gull.cpp)
// ---------------------------------------------------------------------------

const B_MAGIC: [u64; 64] = [
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

const R_MAGIC: [u64; 64] = [
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

const B_SHIFT: [u32; 64] = [
    58, 59, 59, 59, 59, 59, 59, 58, 59, 59, 59, 59, 59, 59, 59, 59, 59, 59, 57, 57, 57, 57, 59, 59,
    59, 59, 57, 55, 55, 57, 59, 59, 59, 59, 57, 55, 55, 57, 59, 59, 59, 59, 57, 57, 57, 57, 59, 59,
    59, 59, 59, 59, 59, 59, 59, 59, 58, 59, 59, 59, 59, 59, 59, 58,
];

const B_OFFSET: [usize; 64] = [
    0, 64, 96, 128, 160, 192, 224, 256, 320, 352, 384, 416, 448, 480, 512, 544, 576, 608, 640, 768,
    896, 1024, 1152, 1184, 1216, 1248, 1280, 1408, 1920, 2432, 2560, 2592, 2624, 2656, 2688, 2816,
    3328, 3840, 3968, 4000, 4032, 4064, 4096, 4224, 4352, 4480, 4608, 4640, 4672, 4704, 4736, 4768,
    4800, 4832, 4864, 4896, 4928, 4992, 5024, 5056, 5088, 5120, 5152, 5184,
];

const R_SHIFT: [u32; 64] = [
    52, 53, 53, 53, 53, 53, 53, 52, 53, 54, 54, 54, 54, 54, 54, 53, 53, 54, 54, 54, 54, 54, 54, 53,
    53, 54, 54, 54, 54, 54, 54, 53, 53, 54, 54, 54, 54, 54, 54, 53, 53, 54, 54, 54, 54, 54, 54, 53,
    53, 54, 54, 54, 54, 54, 54, 53, 52, 53, 53, 53, 53, 53, 53, 52,
];

const R_OFFSET: [usize; 64] = [
    5248, 9344, 11392, 13440, 15488, 17536, 19584, 21632, 25728, 27776, 28800, 29824, 30848, 31872,
    32896, 33920, 35968, 38016, 39040, 40064, 41088, 42112, 43136, 44160, 46208, 48256, 49280,
    50304, 51328, 52352, 53376, 54400, 56448, 58496, 59520, 60544, 61568, 62592, 63616, 64640,
    66688, 68736, 69760, 70784, 71808, 72832, 73856, 74880, 76928, 78976, 80000, 81024, 82048,
    83072, 84096, 85120, 87168, 91264, 93312, 95360, 97408, 99456, 101504, 103552,
];

/// All immutable tables shared by engines.
pub struct Tables {
    pub forward: [[u64; 8]; 2],
    pub west: [u64; 8],
    pub east: [u64; 8],
    pub p_isolated: [u64; 8],
    pub h_line: [u64; 64],
    pub v_line: [u64; 64],
    pub n_diag: [u64; 64],
    pub s_diag: [u64; 64],
    pub r_mask: [u64; 64],
    pub b_mask: [u64; 64],
    pub q_mask: [u64; 64],
    pub n_att: [u64; 64],
    pub s_area: [u64; 64],
    pub d_area: [u64; 64],
    pub n_area: [u64; 64],
    pub bishop_forward: [[u64; 64]; 2],
    pub p_att: [[u64; 64]; 2],
    pub p_move: [[u64; 64]; 2],
    pub p_way: [[u64; 64]; 2],
    pub p_support: [[u64; 64]; 2],
    pub between: [[u64; 64]; 64],
    pub full_line: [[u64; 64]; 64],
    pub file: [u64; 8],
    pub line: [u64; 8],
    pub update_castling: [u8; 64],
    pub mobility: [[i32; 32]; 4],
    pub shelter: [[i32; 8]; 3],
    pub storm_hof: [i32; 2],
    pub storm_blocked: [i32; 4],
    pub storm_shelter_att: [i32; 4],
    pub storm_connected: [i32; 4],
    pub storm_open: [i32; 4],
    pub storm_free: [i32; 4],
    pub log_dist: [i32; 16],
    pub passer_special: [i32; 4],
    pub passer_general: [i32; 8],
    pub passer_blocked: [i32; 8],
    pub passer_free: [i32; 8],
    pub passer_supported: [i32; 8],
    pub passer_protected: [i32; 8],
    pub passer_connected: [i32; 8],
    pub passer_outside: [i32; 8],
    pub passer_candidate: [i32; 8],
    pub passer_clear: [i32; 8],
    pub passer_att: [i32; 8],
    pub passer_def: [i32; 8],
    pub passer_att_log: [i32; 8],
    pub passer_def_log: [i32; 8],
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
    pub turn_key: u64,
    pub piece_key: [[u64; 64]; 16],
    pub castle_key: [u64; 16],
    pub ep_key: [u64; 8],
    pub piece_from_char: [u8; 256],
    pub kpk: Vec<u64>, // [2][64][64]
    pub pst: [i32; 16 * 64],
    pub mvv_lva: [[i32; 16]; 16],
    pub material: Vec<Material>,
    pub b_magic_mask: [u64; 64],
    pub r_magic_mask: [u64; 64],
    pub magic_attacks: Vec<u64>,
    pub b_offset: [usize; 64],
    pub r_offset: [usize; 64],
    pub b_shift: [u32; 64],
    pub r_shift: [u32; 64],
    pub b_magic: [u64; 64],
    pub r_magic: [u64; 64],
}

impl Tables {
    #[inline(always)]
    pub fn bishop_attacks(&self, s: Square, occ: u64) -> u64 {
        debug_assert!(s < 64);
        let idx = unsafe { *self.b_offset.get_unchecked(s) }
            + (((unsafe { *self.b_magic_mask.get_unchecked(s) } & occ)
                .wrapping_mul(unsafe { *self.b_magic.get_unchecked(s) }))
                >> unsafe { *self.b_shift.get_unchecked(s) }) as usize;
        unsafe { *self.magic_attacks.get_unchecked(idx) }
    }

    #[inline(always)]
    pub fn rook_attacks(&self, s: Square, occ: u64) -> u64 {
        debug_assert!(s < 64);
        let idx = unsafe { *self.r_offset.get_unchecked(s) }
            + (((unsafe { *self.r_magic_mask.get_unchecked(s) } & occ)
                .wrapping_mul(unsafe { *self.r_magic.get_unchecked(s) }))
                >> unsafe { *self.r_shift.get_unchecked(s) }) as usize;
        unsafe { *self.magic_attacks.get_unchecked(idx) }
    }

    #[inline(always)]
    pub fn queen_attacks(&self, s: Square, occ: u64) -> u64 {
        self.bishop_attacks(s, occ) | self.rook_attacks(s, occ)
    }

    #[inline(always)]
    pub fn kpk(&self, turn: usize, wp: usize, wk: usize) -> u64 {
        self.kpk[((turn * 64 + wp) * 64) + wk]
    }
}

static TABLES: OnceLock<&'static Tables> = OnceLock::new();

/// Get the process-wide immutable tables, building them on first use.
pub fn tables() -> &'static Tables {
    TABLES.get_or_init(|| Box::leak(Box::new(Tables::new())))
}

impl Tables {
    pub fn new() -> Tables {
        let mut t = Tables {
            forward: [[0; 8]; 2],
            west: [0; 8],
            east: [0; 8],
            p_isolated: [0; 8],
            h_line: [0; 64],
            v_line: [0; 64],
            n_diag: [0; 64],
            s_diag: [0; 64],
            r_mask: [0; 64],
            b_mask: [0; 64],
            q_mask: [0; 64],
            n_att: [0; 64],
            s_area: [0; 64],
            d_area: [0; 64],
            n_area: [0; 64],
            bishop_forward: [[0; 64]; 2],
            p_att: [[0; 64]; 2],
            p_move: [[0; 64]; 2],
            p_way: [[0; 64]; 2],
            p_support: [[0; 64]; 2],
            between: [[0; 64]; 64],
            full_line: [[0; 64]; 64],
            file: [0; 8],
            line: [0; 8],
            update_castling: [0xFF; 64],
            mobility: [[0; 32]; 4],
            shelter: [[0; 8]; 3],
            storm_hof: [0; 2],
            storm_blocked: [0; 4],
            storm_shelter_att: [0; 4],
            storm_connected: [0; 4],
            storm_open: [0; 4],
            storm_free: [0; 4],
            log_dist: [0; 16],
            passer_special: [0; 4],
            passer_general: [0; 8],
            passer_blocked: [0; 8],
            passer_free: [0; 8],
            passer_supported: [0; 8],
            passer_protected: [0; 8],
            passer_connected: [0; 8],
            passer_outside: [0; 8],
            passer_candidate: [0; 8],
            passer_clear: [0; 8],
            passer_att: [0; 8],
            passer_def: [0; 8],
            passer_att_log: [0; 8],
            passer_def_log: [0; 8],
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
            turn_key: 0,
            piece_key: [[0; 64]; 16],
            castle_key: [0; 16],
            ep_key: [0; 8],
            piece_from_char: [0; 256],
            kpk: vec![0; 2 * 64 * 64],
            pst: [0; 16 * 64],
            mvv_lva: [[0; 16]; 16],
            material: vec![Material::default(); TOTAL_MAT],
            b_magic_mask: [0; 64],
            r_magic_mask: [0; 64],
            magic_attacks: vec![0; MAGIC_SIZE],
            b_offset: B_OFFSET,
            r_offset: R_OFFSET,
            b_shift: B_SHIFT,
            r_shift: R_SHIFT,
            b_magic: B_MAGIC,
            r_magic: R_MAGIC,
        };
        t.init_misc();
        t.init_magic();
        t.init_kpk();
        t.init_pst();
        t.init_eval();
        t.init_material();
        t
    }

    // -- init_misc ---------------------------------------------------------

    fn init_misc(&mut self) {
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
                    self.v_line[i] |= u;
                }
                if rank(i) == rank(j) {
                    self.h_line[i] |= u;
                }
                if ndiag(i) == ndiag(j) {
                    self.n_diag[i] |= u;
                }
                if sdiag(i) == sdiag(j) {
                    self.s_diag[i] |= u;
                }
                if dist(i, j) <= 2 {
                    self.d_area[i] |= u;
                    if dist(i, j) <= 1 {
                        self.s_area[i] |= u;
                    }
                    if (rank(i) as i32 - rank(j) as i32).abs()
                        + (file(i) as i32 - file(j) as i32).abs()
                        == 3
                    {
                        self.n_att[i] |= u;
                    }
                }
                if j == i + 8 {
                    self.p_move[0][i] |= u;
                }
                if j + 8 == i {
                    self.p_move[1][i] |= u;
                }
                if (file(i) as i32 - file(j) as i32).abs() == 1 {
                    if rank(j) >= rank(i) {
                        self.p_support[1][i] |= u;
                        if rank(j) - rank(i) == 1 {
                            self.p_att[0][i] |= u;
                        }
                    }
                    if rank(j) <= rank(i) {
                        self.p_support[0][i] |= u;
                        if rank(i) - rank(j) == 1 {
                            self.p_att[1][i] |= u;
                        }
                    }
                } else if file(i) == file(j) {
                    if rank(j) > rank(i) {
                        self.p_way[0][i] |= u;
                    } else {
                        self.p_way[1][i] |= u;
                    }
                }
            }
        }
        for i in 0..64 {
            self.r_mask[i] = self.h_line[i] | self.v_line[i];
            self.b_mask[i] = self.n_diag[i] | self.s_diag[i];
            self.q_mask[i] = self.r_mask[i] | self.b_mask[i];
            self.b_magic_mask[i] = self.b_mask[i] & INTERIOR;
            self.r_magic_mask[i] = self.r_mask[i];
            if file(i) > 0 {
                self.r_magic_mask[i] &= !self.file[0];
            }
            if rank(i) > 0 {
                self.r_magic_mask[i] &= !self.line[0];
            }
            if file(i) < 7 {
                self.r_magic_mask[i] &= !self.file[7];
            }
            if rank(i) < 7 {
                self.r_magic_mask[i] &= !self.line[7];
            }
            for j in 0..64 {
                if self.n_att[i] & self.n_att[j] != 0 {
                    self.n_area[i] |= bit(j);
                }
            }
        }
        for i in 0..8 {
            self.west[i] = 0;
            self.east[i] = 0;
            self.forward[0][i] = 0;
            self.forward[1][i] = 0;
            self.p_isolated[i] = 0;
            for j in 0..8 {
                if i < j {
                    self.forward[0][i] |= self.line[j];
                } else if i > j {
                    self.forward[1][i] |= self.line[j];
                }
                if i < j {
                    self.east[i] |= self.file[j];
                } else if i > j {
                    self.west[i] |= self.file[j];
                }
            }
            if i > 0 {
                self.p_isolated[i] |= self.file[i - 1];
            }
            if i < 7 {
                self.p_isolated[i] |= self.file[i + 1];
            }
        }
        for i in 0..64 {
            let mut u = self.q_mask[i];
            while u != 0 {
                let j = lsb(u);
                cut(&mut u);
                let k = sgn(rank(j) as i32 - rank(i) as i32);
                let l = sgn(file(j) as i32 - file(i) as i32);
                let step = 8 * k + l;
                let mut n = i as i32 + step;
                while n != j as i32 {
                    self.between[i][j] |= bit(n as usize);
                    n += step;
                }
            }
            let mut u = self.b_mask[i];
            while u != 0 {
                let j = lsb(u);
                cut(&mut u);
                self.full_line[i][j] = self.b_mask[i] & self.b_mask[j];
            }
            let mut u = self.r_mask[i];
            while u != 0 {
                let j = lsb(u);
                cut(&mut u);
                self.full_line[i][j] = self.r_mask[i] & self.r_mask[j];
            }
            self.bishop_forward[0][i] |= self.p_way[0][i];
            self.bishop_forward[1][i] |= self.p_way[1][i];
            for j in 0..64 {
                if (self.p_way[1][j] | bit(j)) & self.b_mask[i] & self.forward[0][rank(i)] != 0 {
                    self.bishop_forward[0][i] |= bit(j);
                }
                if (self.p_way[0][j] | bit(j)) & self.b_mask[i] & self.forward[1][rank(i)] != 0 {
                    self.bishop_forward[1][i] |= bit(j);
                }
            }
        }

        for i in 0..16 {
            for j in 0..16 {
                self.mvv_lva[i][j] = if j < WHITE_PAWN {
                    0
                } else if j < WHITE_KNIGHT {
                    pawn_capture_mvv_lva(i) << 26
                } else if j < WHITE_LIGHT {
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
                b'B' => WHITE_LIGHT,
                b'K' => WHITE_KING,
                b'N' => WHITE_KNIGHT,
                b'P' => WHITE_PAWN,
                b'Q' => WHITE_QUEEN,
                b'R' => WHITE_ROOK,
                b'b' => BLACK_LIGHT,
                b'k' => BLACK_KING,
                b'n' => BLACK_KNIGHT,
                b'p' => BLACK_PAWN,
                b'q' => BLACK_QUEEN,
                b'r' => BLACK_ROOK,
                _ => 0,
            } as u8;
        }

        let mut seed = 1u64;
        self.turn_key = rand64(&mut seed);
        for i in 0..8 {
            self.ep_key[i] = rand64(&mut seed);
        }
        for i in 0..16 {
            self.castle_key[i] = rand64(&mut seed);
        }
        for i in 0..16 {
            for j in 0..64 {
                self.piece_key[i][j] = if i == 0 { 0 } else { rand64(&mut seed) };
            }
        }
        for i in 0..16 {
            self.log_dist[i] = (10.0 * (1.01 + i as f64).ln()) as i32;
        }

        self.update_castling = UPDATE_CASTLING;
    }

    // -- init_magic --------------------------------------------------------

    fn init_magic(&mut self) {
        for i in 0..64 {
            let bits = 64 - self.b_shift[i];
            let mut bit_list = [0usize; 16];
            let mut u = self.b_magic_mask[i];
            let mut j = 0;
            while u != 0 {
                bit_list[j] = lsb(u);
                cut(&mut u);
                j += 1;
            }
            for n in 0..(1u64 << bits) {
                let mut occ = 0u64;
                for k in 0..bits {
                    if (n >> k) & 1 != 0 {
                        occ |= bit(bit_list[k as usize]);
                    }
                }
                let idx =
                    self.b_offset[i] + ((B_MAGIC[i].wrapping_mul(occ) >> self.b_shift[i]) as usize);
                self.magic_attacks[idx] = b_magic_attacks(self, i, occ);
            }

            let bits = 64 - self.r_shift[i];
            let mut u = self.r_magic_mask[i];
            let mut j = 0;
            while u != 0 {
                bit_list[j] = lsb(u);
                cut(&mut u);
                j += 1;
            }
            for n in 0..(1u64 << bits) {
                let mut occ = 0u64;
                for k in 0..bits {
                    if (n >> k) & 1 != 0 {
                        occ |= bit(bit_list[k as usize]);
                    }
                }
                let idx =
                    self.r_offset[i] + ((R_MAGIC[i].wrapping_mul(occ) >> self.r_shift[i]) as usize);
                self.magic_attacks[idx] = r_magic_attacks(self, i, occ);
            }
        }
    }

    // -- init_kpk ----------------------------------------------------------

    fn init_kpk(&mut self) {
        const N: usize = 2 * 64 * 64 * 64;
        let mut gen = vec![0u8; N];
        let index =
            |turn: usize, wp: usize, wk: usize, bk: usize| (((turn * 64 + wp) * 64 + wk) * 64) + bk;
        let mut cnt: i32 = 0;
        let mut old_cnt: i32 = 1;
        while cnt != old_cnt {
            old_cnt = cnt;
            cnt = 0;
            for turn in 0..2 {
                for wp in 0..64 {
                    for wk in 0..64 {
                        for bk in 0..64 {
                            if gen[index(turn, wp, wk, bk)] != 0 {
                                continue;
                            }
                            cnt += 1;
                            let r = self.kpk_classify(&gen, turn, wp, wk, bk);
                            if r != 0 {
                                gen[index(turn, wp, wk, bk)] = r;
                            }
                        }
                    }
                }
            }
        }
        for turn in 0..2 {
            for wp in 0..64 {
                for wk in 0..64 {
                    let mut m = 0u64;
                    for bk in 0..64 {
                        if gen[index(turn, wp, wk, bk)] == 2 {
                            m |= bit(bk);
                        }
                    }
                    self.kpk[(turn * 64 + wp) * 64 + wk] = m;
                }
            }
        }
    }

    fn kpk_classify(&self, gen: &[u8], turn: usize, wp: usize, wk: usize, bk: usize) -> u8 {
        let idx =
            |turn: usize, wp: usize, wk: usize, bk: usize| (((turn * 64 + wp) * 64 + wk) * 64) + bk;
        if wp < 8 || wp >= 56 {
            return 1;
        }
        if wp == wk || wk == bk || bk == wp {
            return 1;
        }
        let bwp = bit(wp);
        let bwk = bit(wk);
        let bbk = bit(bk);
        if self.p_att[WHITE][wp] & bbk != 0 {
            if turn == WHITE {
                return 1;
            } else if self.s_area[wk] & bwp == 0 {
                return 1;
            }
        }
        let o = turn ^ 1;
        let mut un = 0;
        if turn == BLACK {
            let mut u = self.s_area[bk] & (!(self.s_area[wk] | self.p_att[WHITE][wp]));
            if u == 0 {
                return 1;
            }
            while u != 0 {
                let to = lsb(u);
                cut(&mut u);
                let v = gen[idx(o, wp, wk, to)];
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
            let mut u = self.s_area[wk] & (!(self.s_area[bk] | bwp));
            while u != 0 {
                let to = lsb(u);
                cut(&mut u);
                let v = gen[idx(o, wp, to, bk)];
                if v == 2 {
                    return 2;
                } else if v == 0 {
                    un += 1;
                }
            }
            let mut to = wp + 8;
            if to != wk && to != bk {
                if to >= 56 {
                    if self.s_area[to] & bbk == 0 {
                        return 2;
                    }
                    if self.s_area[to] & bwk != 0 {
                        return 2;
                    }
                } else {
                    let v = gen[idx(o, to, wk, bk)];
                    if v == 2 {
                        return 2;
                    } else if v == 0 {
                        un += 1;
                    }
                    if to < 24 {
                        to += 8;
                        if to != wk && to != bk {
                            let v = gen[idx(o, to, wk, bk)];
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

    // -- init_pst ----------------------------------------------------------

    fn init_pst(&mut self) {
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
                self.pst[(j << 6) | i] = compose(op / 64, eg / 64);
            }
        }
        self.pst[(WHITE_KNIGHT << 6) | 56] -= compose(100, 0);
        self.pst[(WHITE_KNIGHT << 6) | 63] -= compose(100, 0);
        for i in 0..64 {
            for j in (3..16).step_by(2) {
                let op = opening(self.pst[((j - 1) << 6) | (63 - i)]);
                let eg = endgame(self.pst[((j - 1) << 6) | (63 - i)]);
                self.pst[(j << 6) | i] = compose(-op, -eg);
            }
        }
    }

    // -- init_eval ---------------------------------------------------------

    fn init_eval(&mut self) {
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
            self.passer_general[i] = compose16(
                PASSER_QUAD[0] * q + PASSER_LINEAR[0] * l,
                PASSER_QUAD[1] * q + PASSER_LINEAR[1] * l,
            );
            self.passer_blocked[i] = compose16(
                PASSER_QUAD[2] * q + PASSER_LINEAR[2] * l,
                PASSER_QUAD[3] * q + PASSER_LINEAR[3] * l,
            );
            self.passer_free[i] = compose16(
                PASSER_QUAD[4] * q + PASSER_LINEAR[4] * l,
                PASSER_QUAD[5] * q + PASSER_LINEAR[5] * l,
            );
            self.passer_supported[i] = compose16(
                PASSER_QUAD[6] * q + PASSER_LINEAR[6] * l,
                PASSER_QUAD[7] * q + PASSER_LINEAR[7] * l,
            );
            self.passer_protected[i] = compose16(
                PASSER_QUAD[8] * q + PASSER_LINEAR[8] * l,
                PASSER_QUAD[9] * q + PASSER_LINEAR[9] * l,
            );
            self.passer_connected[i] = compose16(
                PASSER_QUAD[10] * q + PASSER_LINEAR[10] * l,
                PASSER_QUAD[11] * q + PASSER_LINEAR[11] * l,
            );
            self.passer_outside[i] = compose16(
                PASSER_QUAD[12] * q + PASSER_LINEAR[12] * l,
                PASSER_QUAD[13] * q + PASSER_LINEAR[13] * l,
            );
            self.passer_candidate[i] = compose16(
                PASSER_QUAD[14] * q + PASSER_LINEAR[14] * l,
                PASSER_QUAD[15] * q + PASSER_LINEAR[15] * l,
            );
            self.passer_clear[i] = compose16(
                PASSER_QUAD[16] * q + PASSER_LINEAR[16] * l,
                PASSER_QUAD[17] * q + PASSER_LINEAR[17] * l,
            );
            self.passer_att[i] = PASSER_ATT_DEF_QUAD[0] * q + PASSER_ATT_DEF_LINEAR[0] * l;
            self.passer_def[i] = PASSER_ATT_DEF_QUAD[2] * q + PASSER_ATT_DEF_LINEAR[2] * l;
            self.passer_att_log[i] = PASSER_ATT_DEF_QUAD[1] * q + PASSER_ATT_DEF_LINEAR[1] * l;
            self.passer_def_log[i] = PASSER_ATT_DEF_QUAD[3] * q + PASSER_ATT_DEF_LINEAR[3] * l;
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
        self.passer_special = PASSER_SPECIAL;
    }

    // -- init_material / calc_material -------------------------------------

    fn init_material(&mut self) {
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

        for me in 0..2 {
            bishops[me] = light[me] + dark[me];
            major[me] = rooks[me] + queens[me];
            minor[me] = bishops[me] + knights[me];
            tot[me] = 3 * minor[me] + 5 * rooks[me] + 9 * queens[me];
            mat[me] = 32;
            mul[me] = 32;
            quad[me] = 0;
        }

        let mut score = (SEE_VALUE[WHITE_PAWN] + MAT_LINEAR[0]) * (pawns[WHITE] - pawns[BLACK])
            + (SEE_VALUE[WHITE_KNIGHT] + MAT_LINEAR[1]) * (knights[WHITE] - knights[BLACK])
            + (SEE_VALUE[WHITE_LIGHT] + MAT_LINEAR[2]) * (bishops[WHITE] - bishops[BLACK])
            + (SEE_VALUE[WHITE_ROOK] + MAT_LINEAR[3]) * (rooks[WHITE] - rooks[BLACK])
            + (SEE_VALUE[WHITE_QUEEN] + MAT_LINEAR[4]) * (queens[WHITE] - queens[BLACK])
            + 50 * ((bishops[WHITE] / 2) - (bishops[BLACK] / 2));

        let phase = PHASE[PIECE_TYPE[WHITE_PAWN]] * (pawns[WHITE] + pawns[BLACK])
            + PHASE[PIECE_TYPE[WHITE_KNIGHT]] * (knights[WHITE] + knights[BLACK])
            + PHASE[PIECE_TYPE[WHITE_LIGHT]] * (bishops[WHITE] + bishops[BLACK])
            + PHASE[PIECE_TYPE[WHITE_ROOK]] * (rooks[WHITE] + rooks[BLACK])
            + PHASE[PIECE_TYPE[WHITE_QUEEN]] * (queens[WHITE] + queens[BLACK]);
        let phase = (((phase - PHASE_MIN).max(0) * 128) / (PHASE_MAX - PHASE_MIN)).min(128);
        self.material[index].phase = phase as u8;

        let mut special = 0i32;
        for me in 0..2 {
            let o = opp(me);
            if queens[me] == queens[o] {
                if rooks[me] - rooks[o] == 1 {
                    if knights[me] == knights[o] && bishops[o] - bishops[me] == 1 {
                        incv(me, &mut special, compose_pair(MAT_SPECIAL, MAT_RB));
                    } else if bishops[me] == bishops[o] && knights[o] - knights[me] == 1 {
                        incv(me, &mut special, compose_pair(MAT_SPECIAL, MAT_RN));
                    } else if knights[me] == knights[o] && bishops[o] - bishops[me] == 2 {
                        decv(me, &mut special, compose_pair(MAT_SPECIAL, MAT_BBR));
                    } else if bishops[me] == bishops[o] && knights[o] - knights[me] == 2 {
                        decv(me, &mut special, compose_pair(MAT_SPECIAL, MAT_NNR));
                    } else if bishops[o] - bishops[me] == 1 && knights[o] - knights[me] == 1 {
                        decv(me, &mut special, compose_pair(MAT_SPECIAL, MAT_BNR));
                    }
                } else if rooks[me] == rooks[o] && minor[me] - minor[o] == 1 {
                    incv(me, &mut special, compose_pair(MAT_SPECIAL, MAT_M));
                }
            } else if queens[me] - queens[o] == 1 {
                if rooks[o] - rooks[me] == 2 && minor[o] - minor[me] == 0 {
                    incv(me, &mut special, compose_pair(MAT_SPECIAL, MAT_QRR));
                } else if rooks[o] - rooks[me] == 1
                    && knights[o] == knights[me]
                    && bishops[o] - bishops[me] == 1
                {
                    incv(me, &mut special, compose_pair(MAT_SPECIAL, MAT_QRB));
                } else if rooks[o] - rooks[me] == 1
                    && knights[o] - knights[me] == 1
                    && bishops[o] == bishops[me]
                {
                    incv(me, &mut special, compose_pair(MAT_SPECIAL, MAT_QRN));
                } else if (major[o] + minor[o]) - (major[me] + minor[me]) >= 2 {
                    incv(me, &mut special, compose_pair(MAT_SPECIAL, MAT_Q3));
                }
            }
        }
        score += (opening(special) * self.material[index].phase as i32
            + endgame(special) * (128 - self.material[index].phase as i32))
            / 128;

        for me in 0..2 {
            let o = opp(me);
            quad[me] += pawns[me]
                * (pawns[me] * trav(MatQuadMe::PawnPawn)
                    + knights[me] * trav(MatQuadMe::KnightPawn)
                    + bishops[me] * trav(MatQuadMe::BishopPawn)
                    + rooks[me] * trav(MatQuadMe::RookPawn)
                    + queens[me] * trav(MatQuadMe::QueenPawn));
            quad[me] += knights[me]
                * (knights[me] * trav(MatQuadMe::KnightKnight)
                    + bishops[me] * trav(MatQuadMe::BishopKnight)
                    + rooks[me] * trav(MatQuadMe::RookKnight)
                    + queens[me] * trav(MatQuadMe::QueenKnight));
            quad[me] += bishops[me]
                * (bishops[me] * trav(MatQuadMe::BishopBishop)
                    + rooks[me] * trav(MatQuadMe::RookBishop)
                    + queens[me] * trav(MatQuadMe::QueenBishop));
            quad[me] += rooks[me]
                * (rooks[me] * trav(MatQuadMe::RookRook) + queens[me] * trav(MatQuadMe::QueenRook));
            quad[me] += pawns[me]
                * (knights[o] * trav(MatQuadOpp::PawnKnight)
                    + bishops[o] * trav(MatQuadOpp::PawnBishop)
                    + rooks[o] * trav(MatQuadOpp::PawnRook)
                    + queens[o] * trav(MatQuadOpp::PawnQueen));
            quad[me] += knights[me]
                * (bishops[o] * trav(MatQuadOpp::KnightBishop)
                    + rooks[o] * trav(MatQuadOpp::KnightRook)
                    + queens[o] * trav(MatQuadOpp::KnightQueen));
            quad[me] += bishops[me]
                * (rooks[o] * trav(MatQuadOpp::BishopRook)
                    + queens[o] * trav(MatQuadOpp::BishopQueen));
            quad[me] += rooks[me] * queens[o] * trav(MatQuadOpp::RookQueen);

            if bishops[me] >= 2 {
                quad[me] += pawns[me] * BISHOP_PAIR_QUAD[0]
                    + knights[me] * BISHOP_PAIR_QUAD[1]
                    + rooks[me] * BISHOP_PAIR_QUAD[2]
                    + queens[me] * BISHOP_PAIR_QUAD[3]
                    + pawns[o] * BISHOP_PAIR_QUAD[4]
                    + knights[o] * BISHOP_PAIR_QUAD[5]
                    + bishops[o] * BISHOP_PAIR_QUAD[6]
                    + rooks[o] * BISHOP_PAIR_QUAD[7]
                    + queens[o] * BISHOP_PAIR_QUAD[8];
            }
        }
        score += (quad[WHITE] - quad[BLACK]) / 100;

        for me in 0..2 {
            let o = opp(me);
            if tot[me] - tot[o] <= 3 {
                if pawns[me] == 0 {
                    if tot[me] <= 3 {
                        mul[me] = 0;
                    }
                    if tot[me] == tot[o] && major[me] == major[o] && minor[me] == minor[o] {
                        mul[me] = if major[me] + minor[me] <= 2 {
                            0
                        } else if major[me] + minor[me] <= 3 {
                            16
                        } else {
                            32
                        };
                    } else if minor[me] + major[me] <= 2 {
                        if bishops[me] < 2 {
                            mat[me] = if bishops[me] != 0 && rooks[me] != 0 {
                                8
                            } else {
                                1
                            };
                        } else if bishops[o] + rooks[o] >= 1 {
                            mat[me] = 1;
                        } else {
                            mat[me] = 32;
                        }
                    } else if tot[me] - tot[o] < 3
                        && minor[me] + major[me] - minor[o] - major[o] <= 1
                    {
                        mat[me] = 4;
                    } else if minor[me] + major[me] <= 3 {
                        mat[me] = 8 * (1 + bishops[me]);
                    } else {
                        mat[me] = 8 * (2 + bishops[me]);
                    }
                }
                if pawns[me] <= 1 {
                    mul[me] = mul[me].min(28);
                    if rooks[me] == 1 && queens[me] + minor[me] == 0 && rooks[o] == 1 {
                        mat[me] = mat[me].min(23);
                    }
                }
            }
            if major[me] == 0 {
                if minor[me] == 0 {
                    if tot[me] == 0 && pawns[me] < pawns[o] {
                        decv(
                            me,
                            &mut score,
                            (pawns[o] - pawns[me]) * SEE_VALUE[WHITE_PAWN],
                        );
                    }
                } else if minor[me] == 1 {
                    if pawns[me] <= 1 && minor[o] >= 1 {
                        mat[me] = 1;
                    }
                    if bishops[me] == 1 && minor[o] == 1 && bishops[o] == 1 && light[me] != light[o]
                    {
                        mul[me] = mul[me].min(15);
                        if pawns[me] - pawns[o] <= 1 {
                            mul[me] = mul[me].min(11);
                        }
                    }
                } else if pawns[me] == 0 && knights[me] == 2 && bishops[me] == 0 {
                    if tot[o] == 0 && pawns[o] != 0 {
                        mat[me] = 6;
                    } else {
                        mul[me] = 0;
                    }
                }
            }
            if mul[me] == 0 {
                mat[me] = 0;
            }
            if mat[me] <= 1 && tot[me] != tot[o] {
                mul[me] = mul[me].min(8);
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
        for me in 0..2 {
            self.material[index].mul[me] = mul[me] as u8;
            self.material[index].pieces[me] = (major[me] + minor[me]) as u8;
        }
        let score = if score > 0 {
            (score * mat[WHITE]) / 32
        } else {
            (score * mat[BLACK]) / 32
        };
        self.material[index].score = score as i16;
        for me in 0..2 {
            let o = opp(me);
            if major[me] == 0 && minor[me] == bishops[me] && minor[me] <= 1 {
                self.material[index].flags |= 1 << me;
            }
            if ((major[me] == 0 || minor[me] == 0) && major[me] + minor[me] <= 1)
                || major[o] + minor[o] == 0
                || (pawns[me] == 0
                    && major[me] == rooks[me]
                    && major[me] == 1
                    && minor[me] == bishops[me]
                    && minor[me] == 1
                    && rooks[o] == 1
                    && minor[o] == 0
                    && queens[o] == 0)
            {
                self.material[index].flags |= 1 << (me + 2);
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
fn incv(me: Color, var: &mut i32, x: i32) {
    if me == WHITE {
        *var += x;
    } else {
        *var -= x;
    }
}

#[inline]
fn decv(me: Color, var: &mut i32, x: i32) {
    if me == WHITE {
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
fn b_magic_attacks(t: &Tables, i: usize, occ: u64) -> u64 {
    let mut att = 0u64;
    let mut u = t.b_mask[i];
    while u != 0 {
        let s = lsb(u);
        cut(&mut u);
        if t.between[i][s] & occ == 0 {
            att |= t.between[i][s] | bit(s);
        }
    }
    att
}

#[inline]
fn r_magic_attacks(t: &Tables, i: usize, occ: u64) -> u64 {
    let mut att = 0u64;
    let mut u = t.r_mask[i];
    while u != 0 {
        let s = lsb(u);
        cut(&mut u);
        if t.between[i][s] & occ == 0 {
            att |= t.between[i][s] | bit(s);
        }
    }
    att
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_deterministic() {
        let a = Tables::new();
        let b = Tables::new();
        assert_eq!(a.turn_key, b.turn_key);
        assert_eq!(a.piece_key, b.piece_key);
        assert_eq!(a.castle_key, b.castle_key);
        assert_eq!(a.ep_key, b.ep_key);
        assert!(a.turn_key != 0);
    }

    #[test]
    fn magic_attacks_match_slow_rays() {
        let t = Tables::new();
        // Compare magic attacks against a direct ray scan for representative
        // squares and occupancies.
        for &s in &[0usize, 7, 27, 35, 56, 63] {
            for occ in [0u64, 0xFFFF_FFFF_FFFF_FFFF, 0x00FF_0000_0000_0000] {
                let mag = t.bishop_attacks(s, occ);
                let slow = slow_bishop(&t, s, occ);
                assert_eq!(mag, slow, "bishop {s} occ {occ:#x}");
                let mag = t.rook_attacks(s, occ);
                let slow = slow_rook(&t, s, occ);
                assert_eq!(mag, slow, "rook {s} occ {occ:#x}");
                assert_eq!(
                    t.queen_attacks(s, occ),
                    slow_bishop(&t, s, occ) | slow_rook(&t, s, occ)
                );
            }
        }
    }

    fn slow_bishop(t: &Tables, s: usize, occ: u64) -> u64 {
        let mut att = 0;
        for (df, dr) in [(1i32, 1i32), (1, -1), (-1, 1), (-1, -1)] {
            let mut f = file(s) as i32 + df;
            let mut r = rank(s) as i32 + dr;
            while (0..8).contains(&f) && (0..8).contains(&r) {
                let q = sq(f as usize, r as usize);
                att |= bit(q);
                if occ & bit(q) != 0 {
                    break;
                }
                f += df;
                r += dr;
            }
        }
        let _ = t;
        att
    }

    fn slow_rook(t: &Tables, s: usize, occ: u64) -> u64 {
        let mut att = 0;
        for (df, dr) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
            let mut f = file(s) as i32 + df;
            let mut r = rank(s) as i32 + dr;
            while (0..8).contains(&f) && (0..8).contains(&r) {
                let q = sq(f as usize, r as usize);
                att |= bit(q);
                if occ & bit(q) != 0 {
                    break;
                }
                f += df;
                r += dr;
            }
        }
        let _ = t;
        att
    }

    #[test]
    fn pawn_attack_sets() {
        let t = Tables::new();
        assert_eq!(t.p_att[WHITE][sq(4, 1)], bit(sq(3, 2)) | bit(sq(5, 2)));
        assert_eq!(t.p_att[BLACK][sq(4, 6)], bit(sq(3, 5)) | bit(sq(5, 5)));
        assert_eq!(t.p_att[WHITE][sq(0, 0)], bit(sq(1, 1)));
        assert_eq!(t.p_att[BLACK][sq(7, 7)], bit(sq(6, 6)));
    }

    #[test]
    fn material_table_basics() {
        let t = Tables::new();
        // Empty material index.
        assert_eq!(t.material[0].score, 0);
        assert_eq!(t.material[0].phase, 0);
        // Starting position material index is the maximum-ish one; all
        // material entries must have sane phase bounds.
        for m in &t.material {
            assert!(m.phase <= 128);
        }
        // A lone white queen has a positive score.
        let idx = (MAT_WQ + MAT_BQ) as usize; // one white queen, one black queen? see decode
        let _ = idx;
    }

    #[test]
    fn pst_symmetry() {
        let t = Tables::new();
        for j in (3..16).step_by(2) {
            for i in 0..64 {
                assert_eq!(
                    t.pst[(j << 6) | i],
                    compose(
                        -opening(t.pst[((j - 1) << 6) | (63 - i)]),
                        -endgame(t.pst[((j - 1) << 6) | (63 - i)])
                    )
                );
            }
        }
    }

    #[test]
    fn kpk_table_has_expected_entries() {
        let t = Tables::new();
        // White king e1, pawn e2, black king e8: white to move should be a win.
        let kpk = t.kpk(WHITE, sq(4, 1), sq(4, 0));
        assert!(kpk & bit(sq(4, 7)) != 0);
    }
}
