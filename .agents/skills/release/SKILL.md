---
name: release
description: Build and verify an Elmapa distribution locally, publish a main or dev channel build with a changelog from merged PRs, or recover a failed channel release.
---

# Release Elmapa

Use [the release guide](../../../docs/releases.md) for the requested operation. For a local build, use the package target and report the archive path and smoke result. For a channel release, check the current branch, remote destination and commits that the push will publish. Use the release target when the user has authorized that push.

Follow the CI run for the pushed commit through publication. Check the release tag, target commit, prerelease status and the three archives against the checksums. If a job fails, start with its first failing command. Rerun transient failures on the same commit so the workflow can resume its draft.

## Changelog

For a channel release, compare the exact release commit with the preceding published tag on the same channel that is an ancestor of that commit. Use the target’s full history for the first release on a channel.

Look up GitHub’s associated PRs for the commits in that range, including squash and rebase merges. Keep merged PRs whose merge commit is in the range. List each PR once with a link and a short summary of what changed. If the range contains no merged PRs, say so in the changelog.

When preparing a stable release PR, draft the changelog from the candidate `dev` commit and include it in the PR description. After publication, verify the range against the published tag and add or update the `## Changelog` section of the GitHub release description. Preserve the build commit and checksum instructions, and confirm the published description contains the changelog.
