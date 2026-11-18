#!/usr/bin/env python3
"""Prepare an immutable test app with a Launch Services isolation environment.

Launch through CUA/Launch Services; direct executable launches do not apply
LSEnvironment. A fresh bundle path prevents replacing a running Mach-O image.
"""
import argparse
import hashlib
from pathlib import Path
import plistlib
import shutil
import subprocess


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary',type=Path)
    parser.add_argument('bundle',type=Path)
    parser.add_argument('--library',type=Path,required=True)
    parser.add_argument('--trace',type=Path)
    parser.add_argument('--replay',type=Path)
    parser.add_argument('--identifier',default='studio.elmapa.performance')
    parser.add_argument('--name', default='ElmapaTest')
    parser.add_argument('--log', type=Path)
    args=parser.parse_args()
    if not args.name or '/' in args.name or '\0' in args.name: parser.error('App name must be one nonempty filename')
    library=args.library.resolve(strict=True)
    temporary=Path('/private/tmp').resolve()
    if temporary not in library.parents:parser.error('Library must be an existing isolated directory under /private/tmp')
    bundle=args.bundle.resolve()
    if temporary not in bundle.parents:parser.error('Test app must be under /private/tmp')
    if bundle.exists():parser.error('Use a fresh bundle path; running executables are never overwritten')
    executable=bundle/'Contents/MacOS'/args.name
    executable.parent.mkdir(parents=True)
    shutil.copy2(args.binary,executable)
    environment={'ELMAPA_LIBRARY_DIR':str(library)}
    if args.log: environment['ELMAPA_LOG']=str(args.log.resolve())
    if args.trace:
        environment['ELMAPA_TRACE']=str(args.trace.resolve())
        environment['ELMAPA_REQUIRE_PRODUCTION']='1'
    if args.replay:
        environment['ELMAPA_REPLAY']=str(args.replay.resolve())
        environment['ELMAPA_REPLAY_WAIT_FOR_START']='1'
    identity=hashlib.sha256(str(bundle).encode()).hexdigest()[:12]
    with (bundle/'Contents/Info.plist').open('wb') as file:
        plistlib.dump(dict(CFBundleExecutable=args.name,CFBundleIdentifier=args.identifier+'.'+identity,CFBundleName=args.name,CFBundleDisplayName=args.name,CFBundleVersion='1',CFBundleShortVersionString='0.1.0',CFBundlePackageType='APPL',NSHighResolutionCapable=True,LSMinimumSystemVersion='12.0',LSEnvironment=environment),file)
    subprocess.run(['codesign','--force','--deep','--sign','-',str(bundle)],check=True)
    print(f'Prepared {bundle}; Launch Services library: {library}. No app was launched.')


if __name__=='__main__':main()
