#!/usr/bin/env bash
# Emit a reusable run only when the exact commit passed the trusted dev push workflow.
set -euo pipefail
api="repos/${GITHUB_REPOSITORY:?}/actions"
if ! run=$(gh api --method GET "$api/workflows/ci.yml/runs" \
    -f event=push -f branch=dev -f status=success -f head_sha="${GITHUB_SHA:?}" -F per_page=1 \
    --jq '.workflow_runs[] | [.id,.run_number] | @tsv'); then
    echo "Could not look up a verified dev run. Building all targets."
    exit 0
fi
if [[ -z "$run" ]]; then exit 0; fi
read -r id number <<< "$run"
# Each artifact must still exist when the publish job starts.
if ! available=$(gh api "$api/runs/$id/artifacts" --jq \
    '[.artifacts[] | select(.expired == false and (.expires_at | fromdateiso8601) > now + 3600) | .name] | sort | join(" ")'); then
    echo "Could not confirm dev artifacts. Building all targets."
    exit 0
fi
if [[ "$available" == "package-macos-15 package-ubuntu-24.04 package-windows-2025" ]]; then
    echo "preview_run=$id" >> "$GITHUB_OUTPUT"
    echo "preview_tag=dev-$number-${GITHUB_SHA:0:12}" >> "$GITHUB_OUTPUT"
    echo "Reusing packages verified in dev run $id."
fi
