//! Which `serde_json::Map` representation this build resolved, measured from
//! its BEHAVIOUR and announced where a gate can read it (#76).
//!
//! # The hazard
//!
//! `serde_json`'s `preserve_order` feature swaps `Map` from a `BTreeMap`
//! (iterates key-sorted) to an `IndexMap` (iterates in insertion order). Every
//! emitter in this workspace that *walks* a `Map` to build something
//! order-sensitive — generated record field order, decoder field sequence, the
//! MIR literal pool — is walking program STRUCTURE, so that swap changes the
//! bytes of a compiled package and therefore its content hash.
//!
//! **No crate in this workspace asks for the feature.** It arrives entirely
//! through cargo's feature unification, from `agent-client-protocol-schema`
//! sharing the build graph. Two consequences, both measured rather than
//! recalled:
//!
//! ```text
//! cargo test -p aion-package              -> preserve_order OFF
//! cargo test --workspace                  -> preserve_order ON
//! ```
//!
//! Same code, same flags, opposite `Map` semantics — decided by which packages
//! are on the command line. And because it is transitive, it can turn on with
//! **no diff in this repository at all**: a dependency bump anywhere in the tree
//! flips it, and the change to our source is a lockfile line.
//!
//! # 🔴 Why this is a RUNTIME probe and not a `cfg`
//!
//! Every other posture announcement in this workspace keys off `cfg(feature =
//! …)`, and that would be **actively wrong here**. A crate can only `cfg` on its
//! OWN features. The obvious construction — give this crate a
//! `preserve-order = ["serde_json/preserve_order"]` forwarding feature and
//! announce on `cfg(feature = "preserve-order")` — reports correctly when
//! someone enables it explicitly and reports **`false` while the feature is on**
//! in the unification case, which is the entire hazard. It would be a label that
//! lies in exactly the situation it exists to detect.
//!
//! So the posture is established by asking a `Map` what it does. Identity is
//! content, never a label.
//!
//! # Printed is not checked
//!
//! An announcement nothing compares is observability, not a gate. The
//! `posture:serde-json-*` gates in `scripts/battery.sh` assert that the
//! posture-appropriate token appeared and its opposite did not, in both
//! resolutions. Note these run with `-- --nocapture`: unlike a `cfg`-selected
//! announcement, which a runner prints as a test NAME whether or not output is
//! captured, a runtime-detected one has to be printed by the running test.

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value};

    /// Grep tokens for the battery. Deliberately unlovely and unique — they are
    /// a machine interface, and a gate that greps for a phrase someone might
    /// reasonably write in prose is a gate with a false-positive waiting in it.
    const INSERTION_ORDERED: &str = "SERDE_JSON_MAP=insertion_ordered";
    const KEY_SORTED: &str = "SERDE_JSON_MAP=key_sorted";

    /// Two keys, inserted in the order that key-sorting would reverse.
    ///
    /// `beta` before `alpha` is the whole probe: an `IndexMap` hands back
    /// `beta` first because that is when it arrived, a `BTreeMap` hands back
    /// `alpha` first because that is where it sorts.
    fn probe() -> Map<String, Value> {
        let mut probe = Map::new();
        probe.insert("beta".to_owned(), Value::Null);
        probe.insert("alpha".to_owned(), Value::Null);
        probe
    }

    /// Whether `serde_json::Map` iterates in insertion order in THIS build.
    fn map_is_insertion_ordered() -> bool {
        probe().keys().next().map(String::as_str) == Some("beta")
    }

    /// Announces the resolved posture on stdout, for `posture:serde-json-*`.
    ///
    /// Deliberately passes in both resolutions. Neither one is wrong — the
    /// emitters are ordering-independent by construction, which is precisely
    /// the claim the other gates exist to keep true. What must never happen is
    /// the posture changing *unnoticed*.
    #[test]
    fn serde_json_map_ordering_is_announced() {
        if map_is_insertion_ordered() {
            println!(
                // Says WHAT the build is, never how it got that way: the probe
                // reads a `Map`, so it cannot distinguish an explicit
                // `--features serde_json/preserve_order` from unification, and
                // claiming either would be inventing a cause it never observed.
                "{INSERTION_ORDERED} — serde_json::Map is an IndexMap here (preserve_order \
                 resolved ON). Emitters that walk a Map see AUTHORED order."
            );
        } else {
            println!(
                "{KEY_SORTED} — serde_json::Map is a BTreeMap here (preserve_order resolved \
                 OFF). Emitters that walk a Map see SORTED order."
            );
        }
    }

    /// The non-vacuity link, and the reason the ON run is a required gate rather
    /// than a slower copy of the OFF one.
    ///
    /// 🔴 **Under `preserve_order` OFF, every "independent of insertion order"
    /// test in this workspace is VACUOUS.** `Map` is a `BTreeMap`, so a fixture
    /// built `alpha, beta, gamma` and one built `gamma, beta, alpha` are not two
    /// objects — they are the same object, and asserting they serialize alike
    /// compares a value with itself. Those tests read identically whether they
    /// are proving something or nothing.
    ///
    /// This test states the biconditional, so it asserts something real in BOTH
    /// resolutions: the fixtures are distinguishable **if and only if** the
    /// probe reports insertion ordering. Under OFF it pins *why* the sibling
    /// tests are vacuous; under ON it pins that they are not.
    #[test]
    fn insertion_order_is_observable_exactly_when_the_map_preserves_it() {
        let forward = {
            let mut map = Map::new();
            map.insert("alpha".to_owned(), Value::Null);
            map.insert("beta".to_owned(), Value::Null);
            map
        };
        let reversed = probe();

        let distinguishable =
            forward.keys().collect::<Vec<_>>() != reversed.keys().collect::<Vec<_>>();

        assert_eq!(
            distinguishable,
            map_is_insertion_ordered(),
            "the ordering probe and the fixtures disagree about this build. If the probe says \
             key-sorted while two opposite insertion orders are still distinguishable (or vice \
             versa), then every `*_independent_of_insertion_order` test in this workspace is \
             asserting something other than what its name claims."
        );
    }
}
