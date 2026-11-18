# Development workflow

For every new feature or bug fix, complete this workflow:

1. Create a **new Git worktree** from `main` with a dedicated branch named
   `codex/<short-description>` before making implementation changes. Use one
   fresh worktree and branch per feature or fix, even for small changes.
2. Implement the change and run all checks inside that worktree. Follow the
   validation commands in `README.md` and the required checks in
   `.github/workflows/ci.yml`, plus any checks needed for the changed behavior.
   For UI changes, also exercise the affected native app workflow.
3. Fix failures and rerun the affected checks until every required check passes.
   If a check cannot run, report the blocker and keep the branch unmerged.
4. Commit the completed change on its dedicated branch. If `main` has advanced,
   merge its latest changes into the feature branch, resolve conflicts in the
   worktree, and rerun all required checks against the combined result.
5. After all checks pass, merge the branch into local `main`. Preserve unrelated
   work in every checkout; if a dirty `main` checkout prevents a safe merge,
   report the blocker instead of discarding changes.
6. Verify that `main` contains the completed change and report the checks run
   and the merge result. Keep the worktree and branch until the merge succeeds.

Treat the feature or fix as complete only after it is validated and merged into
`main`.
