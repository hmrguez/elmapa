"""Metric definitions: presentation intervals, phase boundaries, idle, overflow."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("performance_analysis", Path(__file__).with_name("analyze-performance.py"))
analysis = importlib.util.module_from_spec(spec)
spec.loader.exec_module(analysis)


def event(name, at_us, duration_us=0, value=0, display_time=None):
    return dict(name=name, at_us=at_us, duration_us=duration_us, value=value, display_time=display_time)


class TraceDefinitions(unittest.TestCase):
    def test_presentations_determine_fps_independently_of_cpu_paint(self):
        trace = {"dropped": 2, "events": [
            event("mark:brush", 0),
            event("native_paint", 1000, duration_us=500),
            event("presented", 2000, display_time=10.0),
            event("presented", 3000, display_time=0.0),
            event("presented", 4000, display_time=10.02),
            event("presented", 5000, display_time=10.04),
            event("mark:idle", 6000),
            event("mark:done", 3_006_000),
        ]}
        output = analysis.analyze(trace)
        brush = output["phases"]["brush"]
        self.assertEqual(output["dropped_events"], 2)
        self.assertEqual(brush["presented_frames"], 3)
        self.assertEqual(brush["first_present_callback_ms"], 2)
        self.assertIsNone(output["phases"]["idle"]["first_present_callback_ms"])
        self.assertAlmostEqual(brush["average_presented_fps"], 50)
        self.assertEqual(brush["timings"]["native_paint"]["median_ms"], .5)
        self.assertEqual(output["phases"]["idle"]["presented_frames"], 0)
        self.assertEqual(output["phases"]["idle"]["observed_duration_ms"], 3000)

    def test_counters_and_observation_order_are_preserved(self):
        output = analysis.analyze({"events": [
            event("sprite_upload_bytes", 2000, value=16),
            event("mark:upload", 1000),
            event("sprite_upload_bytes", 3000, value=32),
        ]})
        self.assertEqual(output["phases"]["upload"]["counters"]["sprite_upload_bytes"], {"max": 32, "last": 32, "sum": 48})


if __name__ == "__main__":
    unittest.main()
