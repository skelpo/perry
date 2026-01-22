//! High-level Intermediate Representation (HIR) for Perry
//!
//! The HIR is a typed, simplified representation of TypeScript code
//! that is easier to analyze and transform than the raw AST.

pub mod ir;
pub mod js_transform;
pub mod lower;
pub mod monomorph;

pub use ir::*;
pub use js_transform::{transform_js_imports, fix_cross_module_native_instances, ExportedNativeInstance};
pub use lower::lower_module;
pub use monomorph::monomorphize_module;
