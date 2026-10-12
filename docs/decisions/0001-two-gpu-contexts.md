# 0001 — Two independent GPU contexts

> **Decision record.** One of a small set of ADRs that explain *why* the
> architecture is the way it is. Each shares one shape: Context, Decision,
> Consequences.

- Status: accepted

## Context

Two subsystems want the GPU: Bevy's renderer and the `cymatrox` solver.
`cymatrox` exposes a `GpuContext` that owns its own wgpu device, created
asynchronously (`GpuContext::new().await`), and runs the solver against it.

The solver also reads results back to the CPU. If it shared Bevy's device and
queue, a continuous readback could stall the renderer and hurt the frame rate.

## Decision

Let the two subsystems own **separate wgpu devices**:

- Bevy's renderer keeps the main thread's device.
- The `cymatrox` `GpuContext` lives on the simulation thread, on its own device.

The two do not share GPU resources; they exchange only CPU-side data.

## Consequences

- The renderer and the solver cannot starve each other's queue.
- CPU↔GPU transfers *between* them are impossible — all communication is
  snapshots over a channel, which is what the architecture wants anyway.
- Two devices cost more memory and initialisation time.
- The coexistence must be **proved**, not assumed: `src/bin/gpu_smoke.rs` brings
  both up in one process and fails if either cannot initialise.

---

[← Previous: Control panel](../ui.md) · [Next: ADR 0002 →](0002-analytic-fluid-figure.md)
