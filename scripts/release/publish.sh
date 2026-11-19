#!/usr/bin/env bash
# Runs on the trusted publication job's Linux runner.
set -euo pipefail
channel=${GITHUB_REF_NAME:?}
case "$channel" in main|dev) ;; *) echo "Unknown channel: $channel" >&2; exit 1 ;; esac
repo=${GITHUB_REPOSITORY:?}
version=${RELEASE_VERSION:?}
run_number=${GITHUB_RUN_NUMBER:?}
tag="$channel-$run_number-${GITHUB_SHA:0:12}"
draft=$(gh release view "$tag" --json isDraft --jq .isDraft) || draft=
if [[ "$draft" == false ]]; then
    echo "$tag is already published."
    exit 0
fi

title="v$version"
prerelease=false
if [[ "$channel" == main ]]; then
    # Older stable releases include their Cargo version in the archive names.
    existing=$(gh api --paginate "repos/$repo/releases?per_page=100" |
        jq -r --arg title "$title" --arg version "$version" '
            .[] | select(.draft == false and .prerelease == false) | . as $release
            | select(.name == $title or .tag_name == $title or
                any(.assets[]?; .name | startswith("Elmapa-" + $version + "-" + $release.tag_name + "-")))
            | .tag_name')
    if [[ -n "$existing" ]]; then
        echo "Stable version $version is already published. Bump the Cargo version before promoting to main." >&2
        echo "$existing" >&2
        exit 1
    fi
else
    build_date=$(gh api "repos/$repo/actions/runs/${GITHUB_RUN_ID:?}" |
        jq -er '.created_at | split("T")[0]')
    title="Elmapa v$version Preview ($build_date, build $run_number)"
    prerelease=true
fi

cd packages
shopt -s nullglob
if [[ -n "${PREVIEW_TAG:-}" ]]; then
    # Only filenames carry the channel label. Preserve every verified archive byte.
    for file in Elmapa-"$version"-"$PREVIEW_TAG"-*; do
        mv -- "$file" "${file/-$PREVIEW_TAG-/-$tag-}"
    done
fi
assets=(
    "Elmapa-$version-$tag-x86_64-unknown-linux-gnu.tar.gz"
    "Elmapa-$version-$tag-aarch64-apple-darwin.zip"
    "Elmapa-$version-$tag-x86_64-pc-windows-msvc.zip"
)
for asset in "${assets[@]}"; do
    if [[ ! -f "$asset" ]]; then
        echo "Missing release archive $asset" >&2
        exit 1
    fi
done
sha256sum "${assets[@]}" > SHA256SUMS
if [[ -z "$draft" ]]; then
    gh release create "$tag" --draft --target "$GITHUB_SHA" --title "$title" \
        --notes "Built from $channel commit $GITHUB_SHA. Verify downloads with SHA256SUMS."
fi
gh release upload "$tag" "${assets[@]}" SHA256SUMS --clobber
gh release edit "$tag" --draft=false --prerelease="$prerelease" --latest=false
