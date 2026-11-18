#!/usr/bin/env bash
# Runs on the trusted publication job's Linux runner.
set -euo pipefail
channel=${GITHUB_REF_NAME:?}
case "$channel" in main|dev) ;; *) echo "Unknown channel: $channel" >&2; exit 1 ;; esac
tag="$channel-${GITHUB_RUN_NUMBER:?}-${GITHUB_SHA:0:12}"
cd packages
shopt -s nullglob
linux=(Elmapa-*-"$tag"-x86_64-unknown-linux-gnu.tar.gz)
mac=(Elmapa-*-"$tag"-aarch64-apple-darwin.zip)
windows=(Elmapa-*-"$tag"-x86_64-pc-windows-msvc.zip)
if (( ${#linux[@]} != 1 || ${#mac[@]} != 1 || ${#windows[@]} != 1 )); then
    echo "Expected one release archive for each platform." >&2
    exit 1
fi
assets=("${linux[0]}" "${mac[0]}" "${windows[0]}")
sha256sum "${assets[@]}" > SHA256SUMS
if draft=$(gh release view "$tag" --json isDraft --jq .isDraft); then
    if [[ "$draft" == false ]]; then
        echo "$tag is already published."
        exit 0
    fi
else
    gh release create "$tag" --draft --target "$GITHUB_SHA" --title "Elmapa $tag" \
        --notes "Built from $channel commit $GITHUB_SHA. Verify downloads with SHA256SUMS."
fi
gh release upload "$tag" "${assets[@]}" SHA256SUMS --clobber
prerelease=false
if [[ "$channel" == dev ]]; then prerelease=true; fi
gh release edit "$tag" --draft=false --prerelease="$prerelease" --latest=false
