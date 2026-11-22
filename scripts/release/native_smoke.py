"""Check a native Elmapa binary with two fresh launches and isolated atlas data."""

import json
import os
from pathlib import Path
import subprocess
import tempfile


def check(binary):
    binary = Path(binary).resolve()
    with tempfile.TemporaryDirectory() as temporary:
        work = Path(temporary).resolve()
        library = work / "library"
        library.mkdir()
        replay = work / "replay.json"
        environment = {**os.environ, "ELMAPA_REPLAY": str(replay), "ELMAPA_LIBRARY_DIR": str(library)}
        environment.pop("ELMAPA_REPLAY_WAIT_FOR_START", None)
        ready = {"action": "ready"}
        exports = [{"action": "export"}, ready, {"action": "export", "format": "png"}]

        def launch(steps):
            replay.write_text(json.dumps([ready, *steps, ready, {"action": "quit"}]))
            subprocess.run([binary], cwd=work, env=environment, check=True, timeout=180)

        launch([
            {"action": "reduced_motion", "enabled": True}, {"action": "create"}, ready,
            {"action": "brush", "brush": "path"}, {"action": "down", "x": 0.42, "y": 0.5},
            {"action": "move", "x": 0.50, "y": 0.55}, {"action": "up", "x": 0.58, "y": 0.5},
            ready, {"action": "save"}, ready, *exports,
        ])
        document = json.loads((library / "native-save.elmapa").read_bytes())
        strokes = [stroke for layer in document["layers"] for stroke in layer["strokes"]]
        if not any(stroke["brush"] == "path" and len(stroke["points"]) >= 2 for stroke in strokes):
            raise ValueError("Replayed pointer gesture did not persist a trail")
        paths = [library / f"native-export.{extension}" for extension in ("svg", "png")]
        before = [path.read_bytes() for path in paths]
        if not before[0].startswith(b"<svg") or not before[1].startswith(b"\x89PNG\r\n\x1a\n"):
            raise ValueError("Native export did not create both image formats")
        for path in paths:
            path.unlink()
        launch([{"action": "open"}, ready, *exports])
        if [path.read_bytes() for path in paths] != before:
            raise ValueError("Reopening the saved atlas changed its exports")
    print("Native edit, save, quit, reopen and SVG and PNG exports verified")
