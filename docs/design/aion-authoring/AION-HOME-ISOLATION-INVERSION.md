# AION_HOME is an isolation inversion (#113)

Found 2026-07-31 while standing up an isolated server for the #97 live proof.
Measured, not inferred. **P0 by blast radius, not by frequency.**

## What happens

Launch a server with `AION_HOME` pointed at a private directory, from a working
directory that happens to contain an `aion-data/` directory. The server logs:

```
INFO  aion-server configuration resolved   aion_home=/private/…/proof97/home
WARN  Aion home legacy-directory migration guard active
      notice=AION HOME MIGRATION REQUIRED: using legacy store data directory
      `/Users/tom/Developer/ablative/stack/aion/aion-data` instead of new
      Aion-home default `/private/…/proof97/home/data`
INFO  aion-server data root resolved       data_root=/Users/tom/Developer/ablative/stack/aion/aion-data
```

It names the hazard, and then does it.

## Why this is the dangerous direction

An operator sets `AION_HOME` for exactly one reason: **to put this server's
state somewhere other than where it would otherwise go.** It is the isolation
control. When a legacy directory exists in the working directory, that control
is silently inverted — the server is pointed at precisely the shared location
the operator was steering away from.

On this box, that location is 6.4G and **Tom's live server holds 66 open
handles under it, including `writer.lock` and shard WALs.**

**The only thing that stopped my proof server opening it was an unrelated
missing feature flag** — `--store-url` selects the libSQL backend, which is not
compiled into the default binary, so the process died before it could touch
anything. Isolation held by luck, not by design.

The constraint that caught it was procedural, not technical: *verify isolation
from the product's own log, never from the flag you passed*. That is the only
reason this is a finding and not an incident.

## Mechanism — confirmed, not guessed

`crates/aion-server/src/config/resolution.rs`, `fill_home_defaults`:

```rust
if config.store.data_dir.is_none() {
    let home_default = home.join(DEFAULT_HAEMATITE_DATA_DIR);
    let legacy = working_dir.join(LEGACY_HAEMATITE_DATA_DIR);
    let selected = select_default("store data", legacy, home_default, &mut migrations)?;
    config.store.data_dir = Some(path_to_string(&selected, "store.data_dir")?);
}
```

`select_default` returns `legacy` whenever that path is a real directory, and
`home_default` otherwise. So an existing `./aion-data` beats the configured
home unconditionally.

Confirmed by mechanism rather than reasoning: relaunching from a directory
containing no `aion-data` resolved the data root correctly to `AION_HOME/data`.
Same binary, same environment, one variable changed.

## The function's own doc comment states the intended rule

> A path is eligible for migration only while its config field is absent. File
> and environment values are therefore final before this function runs, and **an
> explicitly configured value can never activate a legacy fallback.**

That rule is correct and it is correctly implemented — *for `store.data_dir`*.

**The gap is that setting `AION_HOME` IS an explicit configuration of where
state lives, and it does not count.** The guard asks "did you configure the
data directory?" when the operator answered a broader question: "I have told
you where this server's whole home is."

## Why the information is missing

`crates/aion-server/src/config/home.rs`:

```rust
pub fn aion_home() -> Result<PathBuf, ServerError> {
    let configured = std::env::var_os("AION_HOME");
    …
}
```

It reads the variable, branches on it, and returns a bare `PathBuf`. **The
provenance is discarded at the point of resolution**, so `fill_home_defaults`
cannot distinguish "home was explicitly relocated" from "home defaulted to
`$HOME/.aion`". The decision needs a fact that was thrown away one function
earlier — the same shape as #83, where the reason existed and was dropped
before it reached the record.

## The fix

Carry the provenance instead of recomputing or guessing it:

1. `aion_home()` returns the path **and** whether it came from `AION_HOME` or
   was derived from `$HOME`. A small enum (`HomeSource::Explicit` /
   `HomeSource::Derived`) rather than a `bool`, so call sites read correctly.
2. `fill_home_defaults` takes that source. When home is `Explicit`, the legacy
   working-directory fallback **is not eligible at all** — `select_default` is
   never consulted for it and `home_default` is used.
3. When home is `Explicit` **and** a legacy directory exists, keep a log line,
   but change what it says: not "migration required", which reads as advice
   about a future chore, but that the legacy directory was **ignored** because
   `AION_HOME` was set. The operator needs to know their control was honoured
   and that the other directory is still sitting there.
4. The `Derived` path keeps today's behaviour exactly. That is the case the
   migration guard was built for — an operator who has not moved anything and
   whose durable state genuinely lives in `./aion-data`. Nothing about this
   change may strand that state; it is why the guard exists.

This is deletion-class in effect: it removes a reachable state (explicit home,
legacy path chosen) rather than logging it better.

## Tests this needs

- Explicit home + a legacy `aion-data` present in the working directory →
  resolved data root is under the home. **This is the regression pin and it
  fails today.**
- Derived home + legacy present → still the legacy path. The migration guard
  must keep working; a fix that strands durable state is worse than the defect.
- Explicit home + no legacy → home default (unchanged, guards against fixing
  it by disabling the branch).
- The authoring workspace goes through the identical `select_default` path, so
  it is the same inversion with a smaller blast radius. Fix and pin both, and
  say in the test names which is which.

## Note for whoever picks this up

Do not "fix" this by removing the legacy fallback outright. That would silently
strand a real operator's durable state on the next restart, which is a worse
failure than the one being fixed. The fallback is right; **its precedence
against an explicit home is what is wrong.**

---

# BUILT AND CLOSED — 2026-07-31

Built exactly as designed above. The fallback still exists and still protects
the operator it was built for; only its precedence changed.

## What moved

`crates/aion-server/src/config/home.rs`

- `aion_home()` now returns `AionHome { path, source }` where `source` is
  `HomeSource::Explicit` (the operator set `AION_HOME`) or `HomeSource::Derived`
  (`$HOME/.aion`). The provenance is no longer discarded at the point of
  resolution.

`crates/aion-server/src/config/resolution.rs`

- `MigrationNotice` became `LegacyNotice`, carrying a `LegacyDisposition` of
  `Adopted` or `IgnoredForExplicitHome`. **The rename is load-bearing:** the
  vector now holds entries that are not migrations, and calling it `migrations`
  would have been a name that lies. `ConfigResolution.migrations` became
  `legacy_notices` for the same reason.
- `select_default` takes the `HomeSource`. Under an explicit home the legacy
  path is detected, reported, and **not selected**.
- `log_startup` picks the headline from the disposition, so an ignored
  directory no longer reads as a migration chore the operator must go and do.

`crates/aion-server/src/config/load.rs`

- `load_in` threads the source through. `from_slice_with_home` passes
  `Explicit`: an embedded caller that names a home is isolating state, and
  naming it in the call is the same assertion `AION_HOME` makes.
- `load_for_test` takes the source as a **parameter**, not a constant. The two
  worlds are exactly what the guard switches on, so a test has to be able to say
  which one it stands in; a helper that could only simulate one would pin half
  the behaviour and read as covered.

The `#[cfg(not(unix))]` `home_explicit` flag was deliberately **left alone**. It
looks like the same fact and is not: it asks "can this root's ACL be trusted",
for which an in-process supplied home also counts. Merging them would have been
a silent semantic change to a platform this box cannot compile.

## Proof — the pins were watched failing

A green pin proves nothing until it has been seen to fail. The fix was reverted
in place (`select_default` forced back to adopting the legacy directory) and the
suite re-run:

```
an_explicit_home_is_never_overridden_by_a_legacy_working_directory ... FAILED
  left:  …/project/aion-data          ← the defect
  right: …/isolated-home/data         ← the operator's home

aion_home_environment_override_is_respected_everywhere ... FAILED
  left:  …/project/aion-data
  right: …/overridden-home/data
```

The second one is the valuable one: it runs in a **child process with a real
`AION_HOME` set**, through real `aion_home()` and real `ServerConfig::load()`,
with both legacy directories planted in its working directory. It is the
production path, not a helper.

Three controls stayed green through the reverted run, which is what makes the
two failures mean something:

- `a_derived_home_adopts_legacy_directories_and_guards_only_unconfigured_defaults`
  — durable state is still never stranded.
- `legacy_symlink_directories_are_never_selected`.
- `an_explicit_home_with_no_legacy_directory_reports_nothing` — the guard
  against "fixing" this by disabling the branch, which would otherwise pass
  every path assertion above.

The fix was then restored from a scratchpad copy, not `git checkout --`, which
would have reverted the fix along with the mutation.

## Gates

| Gate | Result |
|---|---|
| `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0** |
| `cargo test -p aion-server` | **exit 0, 0 failed** (687 lib + every integration target) |
| `cargo test -p aion-cli --test dev_hot_reload_e2e` | **exit 0, 8 passed** — the other `AION_HOME` consumer |
| `cargo fmt` | applied |

Box measured quiet immediately before the server battery and the witness
written above the run: `cargo=0 rustc=0 nextest=0`, control `zsh=18`.

## What an operator sees now

Explicit home, legacy directory present:

```
WARN  Aion home legacy directory ignored because AION_HOME is set
      notice=AION HOME IS SET, SO THE LEGACY DIRECTORY WAS IGNORED: legacy store
      data directory `…/aion-data` exists but this server uses `…/home/data`;
      the legacy directory is untouched and no state is being read from or
      written to it
INFO  aion-server data root resolved   data_root=…/home/data
```

It still names the other directory — an operator who has one needs to know it is
there — but it no longer names it as the thing being used, because it is not.
