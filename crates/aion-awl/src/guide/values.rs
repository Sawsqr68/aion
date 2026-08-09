//! The words that read values: collection operations, predicates, and the
//! boolean connectives that carry narrowing proofs.

use super::entry::{GuideEntry, STEPS_AND_CALLS, WordKind};

/// `filter`.
pub(crate) const FILTER: GuideEntry = GuideEntry::new(
    "filter",
    WordKind::Reserved,
    "Keeps the elements of a collection its predicate accepts.",
    STEPS_AND_CALLS,
);

/// `map`.
pub(crate) const MAP: GuideEntry = GuideEntry::new(
    "map",
    WordKind::Reserved,
    "Rewrites each element of a collection through a deterministic value expression.",
    STEPS_AND_CALLS,
);

/// `sort`.
pub(crate) const SORT: GuideEntry = GuideEntry::new(
    "sort",
    WordKind::Reserved,
    "Orders a collection by a key, which the compile refuses unless it is `Int`, `Float`, or `String`.",
    STEPS_AND_CALLS,
);

/// `count`.
pub(crate) const COUNT: GuideEntry = GuideEntry::new(
    "count",
    WordKind::Reserved,
    "Counts a collection's elements.",
    STEPS_AND_CALLS,
);

/// `any`.
pub(crate) const ANY: GuideEntry = GuideEntry::new(
    "any",
    WordKind::Reserved,
    "True when at least one element of a collection satisfies its predicate.",
    STEPS_AND_CALLS,
);

/// `all`.
pub(crate) const ALL: GuideEntry = GuideEntry::new(
    "all",
    WordKind::Reserved,
    "True when every element of a collection satisfies its predicate.",
    STEPS_AND_CALLS,
);

/// `is`.
pub(crate) const IS: GuideEntry = GuideEntry::new(
    "is",
    WordKind::Reserved,
    "Opens a predicate: `is empty`, `is present`, or `is absent`.",
    STEPS_AND_CALLS,
);

/// `empty`.
pub(crate) const EMPTY: GuideEntry = GuideEntry::new(
    "empty",
    WordKind::Reserved,
    "The predicate for a collection with nothing in it.",
    STEPS_AND_CALLS,
);

/// `present`.
pub(crate) const PRESENT: GuideEntry = GuideEntry::new(
    "present",
    WordKind::Reserved,
    "Proves an optional access path present, narrowing it from `T?` to `T` wherever that proof holds.",
    STEPS_AND_CALLS,
);

/// `absent`.
pub(crate) const ABSENT: GuideEntry = GuideEntry::new(
    "absent",
    WordKind::Reserved,
    "Proves an optional access path absent, which is what an `or` chain narrows on its false edge.",
    STEPS_AND_CALLS,
);

/// `not`.
pub(crate) const NOT: GuideEntry = GuideEntry::new(
    "not",
    WordKind::Reserved,
    "Negates a boolean expression.",
    STEPS_AND_CALLS,
);

/// `and`.
pub(crate) const AND: GuideEntry = GuideEntry::new(
    "and",
    WordKind::Reserved,
    "Boolean conjunction, where each conjunct's presence proof holds for the operands after it.",
    STEPS_AND_CALLS,
);

/// `or`.
pub(crate) const OR: GuideEntry = GuideEntry::new(
    "or",
    WordKind::Reserved,
    "Boolean disjunction, where each disjunct's absence proof holds on its false edge.",
    STEPS_AND_CALLS,
);
