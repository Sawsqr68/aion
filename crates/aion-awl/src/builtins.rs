//! The single authority for names owned by AWL or reserved by its backend.
//!
//! The roles intentionally differ: every builtin type is referenceable in AWL,
//! while Gleam's boolean constructors are reserved only so generated names
//! cannot collide with the target language. Declaration checking and emission
//! consume their derived union.

/// Leaf types that an AWL document may reference without declaring them.
pub const BUILTIN_TYPES: [&str; 6] = ["Bool", "Int", "Float", "String", "Nil", "Dir"];

/// Gleam constructors that generated type names must not claim.
pub const GLEAM_RESERVED: [&str; 2] = ["True", "False"];

/// Names authors cannot declare and the emitter must pre-claim.
pub const RESERVED_NAMES: [&str; BUILTIN_TYPES.len() + GLEAM_RESERVED.len()] = reserved_names();

const fn reserved_names() -> [&'static str; BUILTIN_TYPES.len() + GLEAM_RESERVED.len()] {
    let mut names = [""; BUILTIN_TYPES.len() + GLEAM_RESERVED.len()];
    let mut source = 0;
    while source < BUILTIN_TYPES.len() {
        names[source] = BUILTIN_TYPES[source];
        source += 1;
    }
    source = 0;
    while source < GLEAM_RESERVED.len() {
        names[BUILTIN_TYPES.len() + source] = GLEAM_RESERVED[source];
        source += 1;
    }
    names
}
