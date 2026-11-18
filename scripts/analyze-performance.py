#!/usr/bin/env python3
"""Summarize opt-in traces; presentation intervals are not CPU paint durations."""
import argparse
import collections
import json
import math
from pathlib import Path


def percentiles(values):
    values = sorted(values)
    def percentile(p):
        return values[min(len(values) - 1, max(0, math.ceil(len(values) * p) - 1))]
    return {"samples": len(values), "median_ms": percentile(.5), "p95_ms": percentile(.95), "p99_ms": percentile(.99), "max_ms": max(values)}


def analyze(trace):
    phases = collections.defaultdict(lambda: {"durations": collections.defaultdict(list), "counters": collections.defaultdict(list), "presented": [], "first_present_callback": None})
    phase = "startup"
    starts = {phase: 0}
    ends = {}
    last_at = 0
    for event in sorted(trace["events"], key=lambda e: e["at_us"]):
        name = event["name"]
        last_at = event["at_us"]
        if name.startswith("mark:"):
            ends[phase] = last_at
            phase = name.removeprefix("mark:")
            starts.setdefault(phase, last_at)
            _ = phases[phase]
            continue
        current = phases[phase]
        if event["duration_us"]:
            current["durations"][name].append(event["duration_us"] / 1000)
        if name == "presented" and event.get("display_time", 0):
            current["presented"].append(event["display_time"])
            if current["first_present_callback"] is None:
                current["first_present_callback"] = event["at_us"]
        elif name not in {"frame_begin", "input"} and not event["duration_us"]:
            current["counters"][name].append(event["value"])
    output = {"dropped_events": trace.get("dropped", 0), "phases": {}}
    for name, data in phases.items():
        result = {"timings": {key: percentiles(values) for key, values in data["durations"].items()}, "counters": {key: {"max": max(values), "last": values[-1], "sum": sum(values)} for key, values in data["counters"].items()}}
        timestamps = sorted(set(data["presented"]))
        intervals = [(b - a) * 1000 for a, b in zip(timestamps, timestamps[1:])]
        if intervals:
            result["presented_intervals"] = percentiles(intervals)
            result["average_presented_fps"] = 1000 * len(intervals) / sum(intervals)
        first = data["first_present_callback"]
        result["first_present_callback_ms"] = None if first is None else (first - starts.get(name, 0)) / 1000
        result["observed_duration_ms"] = (ends.get(name, last_at) - starts.get(name, 0)) / 1000
        result["presented_frames"] = len(timestamps)
        output["phases"][name] = result
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = json.dumps(analyze(json.loads(args.trace.read_text())), indent=2)
    if args.output:
        args.output.write_text(result + "\n")
    else:
        print(result)


if __name__ == "__main__":
    main()
