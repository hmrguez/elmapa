#!/usr/bin/env python3
"""Compare actual drawable presentations; never infer FPS from CPU paint timings."""
import argparse
import gzip
import importlib.util
import json
from pathlib import Path
import statistics
import sys
sys.dont_write_bytecode = True

spec = importlib.util.spec_from_file_location("performance_analysis", Path(__file__).with_name("analyze-performance.py"))
analysis = importlib.util.module_from_spec(spec)
spec.loader.exec_module(analysis)


def read(path):
    path = Path(path)
    with (gzip.open(path, "rt") if path.suffix == ".gz" else path.open()) as stream:
        trace = json.load(stream)
    result = analysis.analyze(trace)
    if result["dropped_events"]:
        raise ValueError(f"{path}: trace overflow")
    if "replay_complete" not in result["phases"]:
        raise ValueError(f"{path}: replay did not finish")
    for phase in ["zoom", "forest", "settled"]:
        if result["phases"][phase].get("presented_frames", 0) < 100:
            raise ValueError(f"{path}: insufficient actual presentations in {phase}")
    return result


def summarize(paths):
    runs = [read(path) for path in paths]
    result = {}
    for phase in ["zoom", "forest", "settled"]:
        values = [run["phases"][phase] for run in runs]
        result[phase] = dict(
            runs=len(runs),
            median_fps=statistics.median(v["average_presented_fps"] for v in values),
            fps_per_run=[v["average_presented_fps"] for v in values],
            median_p95_interval_ms=statistics.median(v["presented_intervals"]["p95_ms"] for v in values),
            median_p99_interval_ms=statistics.median(v["presented_intervals"]["p99_ms"] for v in values),
            presented_frames=sum(v["presented_frames"] for v in values),
        )
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gpui", nargs="+", required=True)
    parser.add_argument("--bevy", nargs="+", required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = {"gpui": summarize(args.gpui), "bevy": summarize(args.bevy)}
    text = json.dumps(result, indent=2) + "\n"
    if args.output:
        args.output.write_text(text)
    print(text, end="")


if __name__ == "__main__":
    main()
