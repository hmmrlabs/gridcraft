# Keeping the fork current

Updated: 2026-10-19. Revision: M1.6.

`.github/workflows/hmmrlabs-upstream.yml` checks `storytold/gridcraft` main every six hours, at minute 17 UTC. It runs only in `hmmrlabs/gridcraft`. GitHub schedules can be delayed; manual dispatch is also available.

If upstream has new commits, the workflow merges into its temporary checkout, runs the full `cargo xtask ci` gate, and pushes the tested merge to main. Conflicts, failing checks, or a concurrent main update stop the run without changing main. It never force-pushes or resolves conflicts automatically. GitHub Actions run logs and summaries identify failures.

The private `hmmrlabs/grid-viewer` repository checks this fork at minute 38 every six hours. It updates both GridCraft dependency pins, resolves the lockfile, runs native/WASM checks and builds the complete site before committing the pin. Deployment is a separate explicit repository-variable gate, pending Cloudflare CI credentials and release verification. See that repository's `docs/upstream-sync.md`.

Checks run inside the updater workflows: pushes made with `GITHUB_TOKEN` do not reliably trigger subsequent push workflows. No cross-repository token or personal token is required for this polling design.

To stop automatic merges, disable “Sync GridCraft upstream” in this fork's Actions page. To retry after a conflict, resolve and test the merge manually, push it, then dispatch the workflow again. Do not bypass the gate to keep a schedule.
