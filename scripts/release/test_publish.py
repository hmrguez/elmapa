"""Exercise release publication with a local stub for the GitHub CLI."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


PUBLISH = Path(__file__).with_name("publish.sh").resolve()
VERSION = "0.2.0"
SHA = "a1b2c3d4e5f6" * 3 + "a1b2"
ARCHIVE_SUFFIXES = (
    "x86_64-unknown-linux-gnu.tar.gz",
    "aarch64-apple-darwin.zip",
    "x86_64-pc-windows-msvc.zip",
)
GH_STUB = """\
import json
import os
from pathlib import Path
import sys

args = sys.argv[1:]
with Path(__file__).with_name("calls.jsonl").open("a") as log:
    print(json.dumps(args), file=log)
if args[:2] == ["release", "view"]:
    draft = os.environ["RELEASE_DRAFT"]
    if not draft:
        sys.exit(1)
    print(draft)
elif args == ["api", "--paginate", "repos/owner/elmapa/releases?per_page=100"]:
    pages = json.loads(os.environ["RELEASE_PAGES"])
    if pages is None:
        sys.exit("Could not list releases")
    for page in pages:
        print(json.dumps(page))
elif args == ["api", "repos/owner/elmapa/actions/runs/123"]:
    print('{"created_at": "2026-11-19T23:45:00Z"}')
elif args[:2] not in (["release", "create"], ["release", "upload"], ["release", "edit"]):
    sys.exit(f"Unexpected GitHub CLI arguments {args!r}")
"""


class PublishTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.packages = self.root / "packages"
        self.packages.mkdir()
        gh = self.root / "gh"
        gh.write_text(f"#!{sys.executable}\n" + GH_STUB)
        gh.chmod(0o755)

    def publish(self, channel="main", *, release_pages=((),), draft="", preview_tag=""):
        tag = f"{channel}-42-{SHA[:12]}"
        for suffix in ARCHIVE_SUFFIXES:
            archive = self.packages / f"Elmapa-{VERSION}-{preview_tag or tag}-{suffix}"
            archive.write_bytes(suffix.encode())
        result = subprocess.run(
            ["bash", PUBLISH],
            cwd=self.root,
            env={
                **os.environ,
                "PATH": f"{self.root}{os.pathsep}{os.environ['PATH']}",
                "GITHUB_REF_NAME": channel,
                "GITHUB_REPOSITORY": "owner/elmapa",
                "GITHUB_RUN_NUMBER": "42",
                "GITHUB_RUN_ID": "123",
                "GITHUB_SHA": SHA,
                "RELEASE_VERSION": VERSION,
                "RELEASE_PAGES": json.dumps(release_pages),
                "RELEASE_DRAFT": draft,
                "PREVIEW_TAG": preview_tag,
            },
            capture_output=True,
            text=True,
        )
        calls = [json.loads(line) for line in (self.root / "calls.jsonl").read_text().splitlines()]
        return result, calls

    def test_preview_title_uses_cargo_version_run_date_and_build_number(self):
        result, calls = self.publish("dev")
        self.assertEqual(result.returncode, 0, result.stderr)
        create = calls[-3]
        self.assertEqual(create[create.index("--title") + 1], f"Elmapa v{VERSION} Preview (2026-11-19, build 42)")
        self.assertIn("--prerelease=true", calls[-1])

    def test_stable_promotion_uses_version_title_and_keeps_preview_archive_bytes(self):
        result, calls = self.publish(preview_tag=f"dev-41-{SHA[:12]}")
        self.assertEqual(result.returncode, 0, result.stderr)
        create = calls[-3]
        self.assertEqual(create[create.index("--title") + 1], f"v{VERSION}")
        self.assertIn("--prerelease=false", calls[-1])
        for suffix in ARCHIVE_SUFFIXES:
            name = f"Elmapa-{VERSION}-main-42-{SHA[:12]}-{suffix}"
            self.assertEqual((self.packages / name).read_bytes(), suffix.encode())
            self.assertIn(name, (self.packages / "SHA256SUMS").read_text())

    def test_legacy_stable_release_reserves_its_version_across_pages(self):
        old_tag = "main-6-123456789abc"
        legacy = {
            "tag_name": old_tag,
            "name": f"Elmapa {old_tag}",
            "draft": False,
            "prerelease": False,
            "assets": [{"name": f"Elmapa-{VERSION}-{old_tag}-{ARCHIVE_SUFFIXES[0]}"}],
        }
        result, calls = self.publish(release_pages=[[{**legacy, "prerelease": True}], [legacy]])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(f"Stable version {VERSION} is already published", result.stderr)
        self.assertFalse(any(call[:2] == ["release", "create"] for call in calls))

    def test_current_stable_release_blocks_the_version_when_archive_list_is_empty(self):
        stable = {
            "tag_name": "main-40-123456789abc",
            "name": f"v{VERSION}",
            "draft": False,
            "prerelease": False,
        }
        result, calls = self.publish(release_pages=[[stable]])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(stable["tag_name"], result.stderr)
        self.assertFalse(any(call[:2] == ["release", "create"] for call in calls))

    def test_release_lookup_failure_prevents_stable_publication(self):
        result, calls = self.publish(release_pages=None)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Could not list releases", result.stderr)
        self.assertEqual(calls[-1][0], "api")

    def test_published_same_run_is_a_no_op(self):
        result, calls = self.publish(draft="false")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([call[:2] for call in calls], [["release", "view"]])

    def test_retry_uploads_to_existing_draft(self):
        tag = f"main-42-{SHA[:12]}"
        draft = {"tag_name": tag, "name": f"v{VERSION}", "draft": True, "prerelease": False}
        result, calls = self.publish(release_pages=[[draft]], draft="true")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([call[:2] for call in calls[-2:]], [["release", "upload"], ["release", "edit"]])


if __name__ == "__main__":
    unittest.main()
