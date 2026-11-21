# Development workflow

For every feature or bug fix:

1. Work on a dedicated branch.
2. Run the validation commands in `README.md`, the applicable checks in
   `.github/workflows/ci.yml`, and any checks needed for the change. For UI
   changes, exercise the affected native app workflow.
3. Commit the change and open a pull request. Fix failures and rerun the
   affected checks. If a required check is blocked, report the blocker and
   leave the pull request unmerged.

Include screenshots of every UI change in the pull request description. If a
change affects a user workflow, also include a screen recording that shows the
updated workflow from start to finish. Use GitHub CLI v2.99.0 or later to upload
the media with `gh pr create --attach <file>` or `gh pr edit --attach <file>`.
Repeat `--attach` for each file.

When working in a linked Git worktree, check the primary checkout for a local
`AGENTS.override.md`. Follow it if present and give it precedence.
