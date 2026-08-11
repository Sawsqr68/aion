# Manifest schema note — named break at v3

The three manifests in this directory do not share a schema, and that is
deliberate, ruled at the r3 land (2026-08-11):

- **`2026-08-11-slice-one-manifest.json` — schema v1.** This is the r1-era
  slice-one evidence exactly as it was derived at its time, restored to its
  original bytes (sha256 `61153ae1a15189405fcc2a51c103bf1a5e71cdf025f91c96c8332f7143d75288`).
  An earlier pass had rewritten it to v3 shape; that rewrite was reverted
  because evidence derived at time T must carry the derivation that existed
  at time T — a manifest regenerated later under a newer schema is a new
  document wearing an old filename.
- **`2026-08-11-r2-manifest.json` and `2026-08-11-r2b-manifest.json` —
  schema v3.** From r2b forward every manifest is derived by the current
  `gate-runner.zsh` (per-leg identity capture, observed-vs-derivation
  binding, tool_versions with known-answer assertions).

Reading rule: compare manifests within a schema generation, not across the
break. The break itself is the record of when the derivation gained the
identity-capture machinery.
