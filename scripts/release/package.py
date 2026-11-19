#!/usr/bin/env python3
"""Build the native release archive and exercise the application extracted from it."""

import argparse
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tarfile
import tempfile
import xml.etree.ElementTree as ET
import zipfile

import native_smoke

ROOT = Path(__file__).resolve().parents[2]
RELEASE = Path(__file__).resolve().parent


def package_version():
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--locked", "--format-version", "1"], cwd=ROOT
    ))
    app = next(package for package in metadata["packages"] if package["name"] == "elmapa")
    return app["version"]


def mac_bundle(binary, destination, version):
    contents = destination / "Contents"
    executable = contents / "MacOS" / "Elmapa"
    executable.parent.mkdir(parents=True, exist_ok=True)
    # A running bundle may still be mapping the old executable.
    replacement = executable.with_suffix(".next")
    shutil.copy2(binary, replacement)
    replacement.replace(executable)
    resources = contents / "Resources"
    resources.mkdir(exist_ok=True)
    shutil.copy2(ROOT / "assets/icons/LICENSE", resources / "ELMAPA-ICONS-LICENSE")
    shutil.copy2(ROOT / "assets/branding/elmapa.icns", resources / "Elmapa.icns")
    (resources / "LUCIDE-LICENSE").unlink(missing_ok=True)
    with (contents / "Info.plist").open("wb") as plist:
        plistlib.dump({
            "CFBundleExecutable": "Elmapa", "CFBundleIdentifier": "studio.elmapa.app",
            "CFBundleName": "Elmapa", "CFBundleDisplayName": "Elmapa",
            "CFBundleIconFile": "Elmapa.icns",
            "CFBundleVersion": version, "CFBundleShortVersionString": version,
            "CFBundlePackageType": "APPL", "NSHighResolutionCapable": True,
            "LSMinimumSystemVersion": "12.0",
        }, plist)
    subprocess.run(["codesign", "--force", "--sign", "-", destination], check=True)
    return executable


def notices(destination, target, offline):
    for filename in ("LICENSE", "THIRD_PARTY_NOTICES.md"):
        shutil.copy2(ROOT / filename, destination / filename)
    notices_dir = destination / "licenses"
    # Preserve the relative links used by the project's asset notices.
    if (ROOT / "licenses").is_dir():
        shutil.copytree(ROOT / "licenses", notices_dir)
    notices_dir.mkdir(exist_ok=True)
    icon_license = destination / "assets/icons/LICENSE"
    icon_license.parent.mkdir(parents=True)
    shutil.copy2(ROOT / "assets/icons/LICENSE", icon_license)
    subprocess.run([
        "cargo-about", "generate", "--locked", "--fail", "--target", target,
        "--config", RELEASE / "about.toml", RELEASE / "licenses.txt.hbs",
        "--output-file", notices_dir / "rust-dependencies.txt", *offline,
    ], cwd=ROOT, check=True)


def archive_package(stage, archive):
    if sys.platform == "darwin":
        subprocess.run(["ditto", "-c", "-k", "--keepParent", stage, archive], check=True)
    elif sys.platform == "win32":
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
            for path in sorted(stage.rglob("*")):
                if path.is_file():
                    bundle.write(path, path.relative_to(stage.parent))
    else:
        with tarfile.open(archive, "w:gz") as bundle:
            bundle.add(stage, arcname=stage.name)


def smoke(archive, unpack, executable, native):
    if sys.platform == "darwin":
        subprocess.run(["ditto", "-x", "-k", archive, unpack], check=True)
        subprocess.run(["codesign", "--verify", "--strict", unpack / "Elmapa/Elmapa.app"], check=True)
    elif sys.platform == "win32":
        with zipfile.ZipFile(archive) as bundle:
            bundle.extractall(unpack)
    else:
        with tarfile.open(archive) as bundle:
            bundle.extractall(unpack, filter="data")
    output = unpack / "map.svg"
    subprocess.run([unpack / executable, "--export-demo", output], cwd=unpack, check=True, timeout=120)
    svg = ET.parse(output).getroot()
    if svg.tag != "{http://www.w3.org/2000/svg}svg" or not len(svg):
        raise ValueError("Packaged application did not export a populated SVG")
    if native:
        native_smoke.check(unpack / executable)


def build_binary(flags):
    # Cargo reports the executable path even with a custom target directory.
    messages = subprocess.check_output([
        "cargo", "build", "--bin", "elmapa", "--message-format=json-render-diagnostics", *flags,
    ], cwd=ROOT, text=True).splitlines()
    return next(Path(item["executable"]) for item in map(json.loads, messages)
                  if item.get("executable") and item["target"]["name"] == "elmapa")


def package(offline, native):
    version = package_version()
    rust_info = subprocess.check_output(["rustc", "-vV"], text=True).splitlines()
    host = next(line.removeprefix("host: ") for line in rust_info if line.startswith("host: "))
    packages = ROOT / "target/packages"
    packages.mkdir(parents=True, exist_ok=True)
    suffix = "tar.gz" if sys.platform.startswith("linux") else "zip"
    identifier = version
    if os.environ.get("GITHUB_ACTIONS") == "true":
        ref = os.environ["GITHUB_REF"]
        channel = {"refs/heads/main": "main", "refs/heads/dev": "dev"}.get(ref, "ci")
        run_number = int(os.environ["GITHUB_RUN_NUMBER"])
        short_sha = os.environ["GITHUB_SHA"][:12]
        identifier = f"{version}-{channel}-{run_number}-{short_sha}"
    archive_name = f"Elmapa-{identifier}-{host}.{suffix}"
    with tempfile.TemporaryDirectory(dir=packages) as temporary:
        workspace = Path(temporary)
        stage = workspace / "Elmapa"
        stage.mkdir()
        bundle = stage / "Elmapa.app"
        notice_folder = bundle / "Contents/Resources" if sys.platform == "darwin" else stage
        notice_folder.mkdir(parents=True, exist_ok=True)
        notices(notice_folder, host, offline)
        binary = build_binary(["--locked", "--release", *offline])
        if sys.platform == "darwin":
            executable = mac_bundle(binary, bundle, version)
        else:
            executable = stage / binary.name
            shutil.copy2(binary, executable)
        archive = workspace / archive_name
        archive_package(stage, archive)
        smoke(archive, workspace / "unpacked", executable.relative_to(workspace), native)
        output = packages / archive_name
        archive.replace(output)
    print(f"Verified {output}")


def main():
    if sys.argv[1:2] == ["bundle"]:
        # The development bundle command forwards the caller's Cargo build flags.
        if sys.platform != "darwin":
            raise SystemExit("The application bundle requires macOS")
        binary = build_binary(sys.argv[2:])
        bundle = ROOT / "target/Elmapa.app"
        mac_bundle(binary, bundle, package_version())
        print(f"Built {bundle}")
        return
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--offline", action="store_true", help="use cached crates and license data")
    parser.add_argument("--native", action="store_true", help="also launch and edit in a desktop session")
    parser.add_argument("--print-version", action="store_true", help="print the Cargo package version and exit")
    args = parser.parse_args()
    if args.print_version:
        print(package_version())
        return
    package(["--offline"] if args.offline else [], args.native)


if __name__ == "__main__":
    main()
