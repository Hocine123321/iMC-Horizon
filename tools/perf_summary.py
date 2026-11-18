#!/usr/bin/env python3
"""Summarize an iMC session-log.csv for the performance plan (docs/IOS_PERFORMANCE_PLAN.md).

    python3 tools/perf_summary.py session-log.csv [--from SECONDS] [--to SECONDS]

Reads the 5-second windows the app writes (docs/SESSION_LOG.md), keeps the ones where the engine drew frames
(foreground, frames_per_s not empty), and prints what the plan compares between runs: how fast, how smooth, how much
memory, how hot, how much battery. Windows are weighted by their frames for the averages, so a quiet window does not count
as much as a busy one. Standard library only.
"""
import argparse
import csv
import sys

THERMAL = {"nominal": 0, "fair": 1, "serious": 2, "critical": 3}


def num(row, key):
    v = row.get(key, "")
    try:
        return float(v) if v != "" else None
    except ValueError:
        return None


def read(path):
    with open(path, newline="") as f:
        lines = [l for l in f if not l.startswith("#") and l.strip()]
    return list(csv.DictReader(lines))


def percentile(values, p):
    values = sorted(values)
    if not values:
        return None
    return values[min(len(values) - 1, max(0, int(round(p * (len(values) - 1)))))]


def summarize(rows):
    play = [r for r in rows if num(r, "frames_per_s") is not None]
    if not play:
        return None
    fps = [num(r, "frames_per_s") for r in play]
    avg_ms = [num(r, "frame_avg_ms") for r in play]
    worst = [num(r, "frame_worst_ms") for r in play]
    # Each window is 5 seconds, so frames = fps * 5 (the log does not store the count itself).
    frames = [f * 5 for f in fps]
    total = sum(frames)
    over17 = sum(num(r, "frames_over_17ms") or 0 for r in play)
    over34 = sum(num(r, "frames_over_34ms") or 0 for r in play)
    foot = [num(r, "footprint_mb") for r in play if num(r, "footprint_mb") is not None]
    avail = [num(r, "available_mb") for r in play if num(r, "available_mb") is not None]
    bat = [num(r, "battery_pct") for r in play if num(r, "battery_pct") is not None]
    secs = [num(r, "seconds") for r in play]
    thermal = max((THERMAL.get(r.get("thermal", ""), 0) for r in play), default=0)
    start_thermal = THERMAL.get(play[0].get("thermal", ""), 0)
    names = {v: k for k, v in THERMAL.items()}
    minutes = (secs[-1] - secs[0]) / 60 if len(secs) > 1 else 0
    out = {
        "windows": len(play),
        "minutes": round(minutes, 1),
        "fps_weighted_avg": round(sum(f * w for f, w in zip(fps, frames)) / total, 1) if total else None,
        "fps_worst_window": round(min(fps), 1),
        "fps_5th_percentile_window": round(percentile(fps, 0.05), 1),
        "frame_avg_ms": round(sum(a * w for a, w in zip(avg_ms, frames)) / total, 2) if total else None,
        "frame_worst_ms": round(max(worst), 1),
        "pct_frames_over_17ms": round(100 * over17 / total, 2) if total else None,
        "pct_frames_over_34ms": round(100 * over34 / total, 2) if total else None,
        "footprint_mb_start_end_peak": (foot[0], foot[-1], max(foot)) if foot else None,
        "available_mb_lowest": min(avail) if avail else None,
        "thermal_start_peak": (names[start_thermal], names[thermal]),
        "battery_pct_start_end": (bat[0], bat[-1]) if bat else None,
    }
    if bat and minutes >= 1:
        out["battery_drop_per_10_min"] = round((bat[0] - bat[-1]) * 10 / minutes, 1)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("log")
    ap.add_argument("--from", dest="lo", type=float, default=None)
    ap.add_argument("--to", dest="hi", type=float, default=None)
    a = ap.parse_args()
    rows = read(a.log)
    if a.lo is not None:
        rows = [r for r in rows if (num(r, "seconds") or 0) >= a.lo]
    if a.hi is not None:
        rows = [r for r in rows if (num(r, "seconds") or 0) <= a.hi]
    result = summarize(rows)
    if result is None:
        print("No windows with engine frames (frames_per_s is empty everywhere): was the game running in the foreground, "
              "and is this a log from a build with the frame columns (2026-11-17 or later)?", file=sys.stderr)
        sys.exit(1)
    for k, v in result.items():
        print(f"{k:32} {v}")


if __name__ == "__main__":
    main()
