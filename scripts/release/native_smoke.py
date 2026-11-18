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

        def launch(steps):
            replay.write_text(json.dumps([ready, *steps, ready, {"action": "quit"}]))
            subprocess.run([binary], cwd=work, env=environment, check=True, timeout=180)

        launch([
            {"action": "reduced_motion", "enabled": True}, {"action": "create"}, ready,
            {"action": "brush", "brush": "path"}, {"action": "down", "x": 0.42, "y": 0.5},
            {"action": "move", "x": 0.50, "y": 0.55}, {"action": "up", "x": 0.58, "y": 0.5},
            ready, {"action": "save"}, ready, {"action": "export"},
        ])
        document = json.loads((library / "native-save.elmapa").read_bytes())
        strokes = [stroke for layer in document["layers"] for stroke in layer["strokes"]]
        if not any(stroke["brush"] == "path" and len(stroke["points"]) >= 2 for stroke in strokes):
            raise ValueError("Replayed pointer gesture did not persist a trail")
        export = library / "native-export.svg"
        before = export.read_bytes()
        export.unlink()
        launch([{"action": "open"}, ready, {"action": "export"}])
        if not before or export.read_bytes() != before:
            raise ValueError("Reopening the saved atlas changed its export")
    print("Native edit, save, quit, reopen and export verified")
