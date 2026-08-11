"""M4: the absolute-path doctrine applies to `binary` too, and m3: no `_queue` rename."""

import pathlib

p = pathlib.Path("crates/aion-awl/src/checker/harness.rs")
s = p.read_text()

s = s.replace(
    """/// The `norn` kind's own settings.
///
/// Norn's argv is the adapter's, so there is no `command` to declare and no `cwd` and no
/// permission policy: those are ACP's, and one written here would configure nothing.
/// `binary` is genuinely optional — absent, the adapter uses its own documented
/// `norn`-on-`PATH` default, which is the library's value and not one invented here.
fn check_norn(_queue: &str, harness: &HarnessDecl, refusals: &mut Vec<(Span, String)>) {""",
    """/// The `norn` kind's own settings.
///
/// Norn's argv is the adapter's, so there is no `command` to declare and no `cwd` and no
/// permission policy: those are ACP's, and one written here would configure nothing.
/// `binary` is genuinely optional — absent, the adapter uses its own documented
/// `norn`-on-`PATH` default, which is the library's value and not one invented here.
///
/// Takes no queue name, unlike [`check_acp`]: every refusal it can raise anchors on the
/// setting's own value rather than on the section, so none of them needs to name the
/// queue to be findable. A parameter accepted and then ignored — or renamed to `_queue`
/// to quieten the compiler — would be a lie about what this function reads.
fn check_norn(harness: &HarnessDecl, refusals: &mut Vec<(Span, String)>) {""",
)

s = s.replace(
    """        Some(HARNESS_KIND_NORN) => check_norn(queue, harness, &mut refusals),""",
    """        Some(HARNESS_KIND_NORN) => check_norn(harness, &mut refusals),""",
)

s = s.replace(
    """    if let Some(binary) = &harness.binary
        && binary.value.value.trim().is_empty()
    {
        refusals.push((
            binary.value.span,
            format!(
                "`{HARNESS_BINARY_FIELD}` is empty; omit it to take the adapter's own \\
                 `{HARNESS_KIND_NORN}`-on-`PATH` default, or name the binary"
            ),
        ));
    }
}""",
    """    // The same absolute-path doctrine `command` and `cwd` live by, for the same reason
    // verbatim: a worker is started from whatever directory a supervisor or a shell
    // happens to be in, so a relative path names a different file on every launch. There
    // is no PATH-lookup exception here — a section that WANTS the adapter's
    // `norn`-on-`PATH` default omits `binary` entirely, which says so plainly, whereas
    // `binary "norn"` would be a second spelling of the same thing and a bare
    // `binary "bin/norn"` would be a hazard wearing its clothes.
    if let Some(binary) = &harness.binary {
        let value = binary.value.value.as_str();
        if value.trim().is_empty() {
            refusals.push((
                binary.value.span,
                format!(
                    "`{HARNESS_BINARY_FIELD}` is empty; omit it to take the adapter's own \\
                     `{HARNESS_KIND_NORN}`-on-`PATH` default, or name the binary"
                ),
            ));
        } else if !std::path::Path::new(value).is_absolute() {
            refusals.push((
                binary.value.span,
                format!(
                    "`{HARNESS_BINARY_FIELD} \\"{value}\\"` must be an ABSOLUTE path. A worker \\
                     is started from whatever directory a supervisor or a shell happens to \\
                     be in, so a relative path names a different file on every launch. To \\
                     take the adapter's own `{HARNESS_KIND_NORN}`-on-`PATH` default instead, \\
                     omit `{HARNESS_BINARY_FIELD}` entirely."
                ),
            ));
        }
    }
}""",
)
p.write_text(s)
print("check_norn takes no queue:", "fn check_norn(harness: &HarnessDecl" in s)
print("absolute rule added:", "must be an ABSOLUTE path. A worker" in s)
print("_queue gone:", "_queue" not in s)
