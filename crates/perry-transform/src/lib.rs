//! IR Transformations for Perry
//!
//! This crate contains transformation passes that run on the HIR:
//! - Closure conversion
//! - Async/await lowering
//! - Optimization passes (function inlining)

pub mod closure;
pub mod inline;

// Re-export main transformation functions
pub use closure::convert_closures;
pub use inline::inline_functions;
