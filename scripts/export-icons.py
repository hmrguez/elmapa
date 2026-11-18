#!/usr/bin/env python3
"""Validate and export the icon sources, using only the Python standard library."""

import argparse
import html
import io
import json
from pathlib import Path
import sys
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "assets/icons"
DESTINATION = ROOT / "assets/icon-pack"
NAMESPACE = "{http://www.w3.org/2000/svg}"


def export():
    manifest_bytes = (SOURCE / "manifest.json").read_bytes()
    manifest = json.loads(manifest_bytes)
    icons = manifest["icons"]
    names = [icon["name"] for icon in icons]
    assert len(names) == len(set(names)), "Duplicate icon names"
    assert {icon["file"] for icon in icons} == {
        file.name for file in SOURCE.glob("*.svg")
    }, "Manifest must cover exactly the source SVGs"
    attributes = {
        "viewBox": "0 0 24 24",
        "fill": "none",
        "stroke": "currentColor",
        "stroke-width": "2",
        "stroke-linecap": "round",
        "stroke-linejoin": "round",
    }
    symbols, cards, files = [], [], {}
    for icon in icons:
        raw = (SOURCE / icon["file"]).read_bytes()
        tree = ET.fromstring(raw)
        assert tree.tag == NAMESPACE + "svg", icon["name"]
        assert all(tree.get(key) == value for key, value in attributes.items()), icon["name"]
        assert len(tree), f"Empty icon: {icon['name']}"
        for element in tree.iter():
            assert element.tag in {NAMESPACE + tag for tag in ("svg", "path", "circle", "rect")}
            assert not any("href" in key or key.startswith("on") for key in element.attrib)
        body = raw.decode().split(">", 1)[1].rsplit("</svg>", 1)[0].strip()
        attrs = " ".join(f'{key}="{value}"' for key, value in attributes.items())
        symbols.append(f'  <symbol id="elmapa-{icon["name"]}" {attrs}>{body}</symbol>')
        samples = "".join(
            f'<span class="sample"><span style="width:{size}px;height:{size}px">'
            f'{raw.decode()}</span><small>{size}</small></span>'
            for size in (17, 24, 40)
        )
        cards.append(
            f'<article><div class="samples">{samples}</div>'
            f'<h3>{html.escape(icon["label"])}</h3><code>{icon["name"]}</code>'
            f'<p>{html.escape(" · ".join(icon["uses"]))}</p></article>'
        )
        files[f'svg/{icon["file"]}'] = raw
    sprite = '<svg xmlns="http://www.w3.org/2000/svg">\n' + "\n".join(symbols) + "\n</svg>\n"
    catalog = """<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>Elmapa Atlas Icons</title>
<style>
:root{color-scheme:light dark;--paper:#f9faf4;--ink:#303e33;--line:#d9dfd1;--quiet:#60715d}
*{box-sizing:border-box}body{margin:0;background:var(--paper);color:var(--ink);font:16px 'Avenir Next',sans-serif}
body.dark{--paper:#242424;--ink:#f5f5f5;--line:#404040;--quiet:#b5b5b5}
header,main,footer{max-width:1200px;margin:auto;padding:40px}header{border-bottom:1px solid var(--line)}
.eyebrow{letter-spacing:.2em;font-size:11px;text-transform:uppercase}h1{font:56px Baskerville,Georgia,serif;margin:18px 0}
header p{max-width:670px;line-height:1.6;color:var(--quiet)}button{background:transparent;color:inherit;border:1px solid var(--line);border-radius:6px;padding:10px 16px;font:inherit;cursor:pointer}
main{display:grid;grid-template-columns:repeat(auto-fit,minmax(210px,1fr));gap:1px;background:var(--line);padding:1px}
article{background:var(--paper);padding:26px 24px;min-width:0}.samples{display:flex;align-items:end;gap:28px;height:75px}
.sample{display:flex;flex-direction:column;gap:12px;align-items:center}.sample svg{display:block;width:100%;height:100%}
small{font-size:10px;color:var(--quiet)}h3{font-size:14px;margin:28px 0 8px}code{font-size:11px;color:var(--quiet)}
article p{font-size:12px;line-height:1.6;color:var(--quiet);min-height:38px}footer{font-size:12px;color:var(--quiet);line-height:1.6}
@media(max-width:600px){header,footer{padding:24px}h1{font-size:40px}}
</style>
<header><div class="eyebrow">Elmapa / Instrument series / 01</div><h1>Atlas Icons</h1>
<p>ICON_COUNT precise symbols for making worlds. Coastlines, groves, ridges and waterways,
drawn on one 24-unit grid. Two-unit strokes. One shared visual language.</p>
<button id="theme" type="button" aria-pressed="false">Dark surface</button></header>
<main>CARDS</main><footer>Version 1.0.0 · MIT · Original Elmapa artwork.<br>
Each row shows 17 px (native), 24 px and 40 px. SVGs inherit currentColor.
Individual SVG files, sprite.svg, manifest.json and LICENSE are included in the pack.</footer>
<script>const button=document.querySelector('#theme');button.onclick=()=>{const dark=document.body.classList.toggle('dark');button.setAttribute('aria-pressed',String(dark));button.textContent=dark?'Light surface':'Dark surface'};</script>
</html>
""".replace("CARDS", "\n".join(cards)).replace("ICON_COUNT", str(len(icons)))
    files.update({
        "sprite.svg": sprite.encode(),
        "catalog.html": catalog.encode(),
        "manifest.json": manifest_bytes,
        "LICENSE": (SOURCE / "LICENSE").read_bytes(),
        "README.md": (SOURCE / "README.md").read_bytes(),
    })
    name = f'elmapa-atlas-icons-{manifest["version"]}'
    buffer = io.BytesIO()
    # Stored entries avoid compression-library differences between CI machines.
    with zipfile.ZipFile(buffer, "w", compression=zipfile.ZIP_STORED) as archive:
        for path, data in sorted(files.items()):
            entry = zipfile.ZipInfo(f"{name}/{path}", (2026, 1, 1, 0, 0, 0))
            entry.compress_type = zipfile.ZIP_STORED
            entry.create_system = 3
            entry.external_attr = 0o100644 << 16
            archive.writestr(entry, data)
    return {"sprite.svg": sprite.encode(), "catalog.html": catalog.encode(), f"{name}.zip": buffer.getvalue()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Validate source icons and build exports in memory")
    args = parser.parse_args()
    outputs = export()
    if args.check:
        print(f"Validated icon manifest, source SVGs and {len(outputs)} generated exports")
    else:
        DESTINATION.mkdir(parents=True, exist_ok=True)
        for name, data in outputs.items():
            (DESTINATION / name).write_bytes(data)
        print(f"Icon pack written to {DESTINATION}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
