# 0003 — Keep the `cymatrox` crate unmodified

- Status: accepted

## Context

The viewer needs behaviour the crate does not expose: a mode-change signal to
schedule the granular rescatter, and a non-stalling fluid figure. The easy path
would be to fork the crate or patch it in place.

## Decision

Treat `cymatrox` as an external dependency. **No changes to it from this
repository.** Where a behaviour is missing, adapt on the viewer side:

- Mirror the crate's documented mode selection locally (`resolved_auto_mode`) to
  notice when a retune lands on a different mode and schedule a rescatter.
- Compute the Fluid figure analytically (see
  [0002](0002-analytic-fluid-figure.md)) instead of extending the crate.

## Consequences

- The viewer tracks the published crate and can take upstream updates.
- The viewer carries a small amount of duplicated logic — the mode-scaling
  formula — confined to one function and documented as such.
- Upstream-internal values are not available; the viewer must not depend on
  them, only on documented behaviour.

---

[← Previous: ADR 0002](0002-analytic-fluid-figure.md) · [Next: Glossary →](../glossary.md)
