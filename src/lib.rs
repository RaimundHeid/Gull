//! A Rust port of the Gull 3 chess engine, derived from the LazyGull C++
//! implementation in `src/`.
//!
//! The code is split into three primary modules:
//!
//! * [`evaluation`] -- the hand-tuned evaluation function, an exact port of
//!   `evaluation()` and its helpers.
//! * [`search`] -- the alpha/beta search, quiescence search, move ordering,
//!   hash tables and iterative deepening root, an exact port of the C++
//!   search.
//! * [`communication`] -- the UCI protocol layer.
//!
//! Supporting modules [`types`], [`tables`] and [`position`] provide the
//! board representation, move encoding and the pre-computed data tables.
//!
//! The port is single threaded and does not include tablebase support.

pub mod communication;
pub mod evaluation;
pub mod position;
pub mod search;
pub mod tables;
pub mod types;

pub use tables::Tables;
pub use types::*;
