---
name: release
description: Build and verify an Elmapa distribution locally, publish a main or dev channel build, or recover a failed channel release.
---

# Release Elmapa

Use [the release guide](../../../docs/releases.md) for the requested operation. For a local build, use the package target and report the archive path and smoke result. For a channel release, check the current branch, remote destination and commits that the push will publish. Use the release target when the user has authorized that push.

Follow the CI run for the pushed commit through publication. Check the release tag, target commit, prerelease status and the three archives against the checksums. If a job fails, start with its first failing command. Rerun transient failures on the same commit so the workflow can resume its draft.
