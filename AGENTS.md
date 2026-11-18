# Development workflow

For every feature or bug fix:

1. Work on a dedicated branch.
2. Run the validation commands in `README.md`, the applicable checks in
   `.github/workflows/ci.yml`, and any checks needed for the change. For UI
   changes, exercise the affected native app workflow.
3. Commit the change and open a pull request. Fix failures and rerun the
   affected checks. If a required check is blocked, report the blocker and
   leave the pull request unmerged.

When working in a linked Git worktree, check the primary checkout for a local
`AGENTS.override.md`. Follow it if present and give it precedence.
