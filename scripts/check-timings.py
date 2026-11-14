#!/usr/bin/env python3
"""Prints how long each step of scripts/check.sh took, and the slowest tests.

check.sh writes one line per step (and per substep) to <target>/check-timings/current.tsv:

    <seconds> TAB <step|sub|failed> TAB <title>

A "sub" line belongs to the step written after it. This script sorts the steps by duration, keeps
each step's substeps under it, adds the slowest tests from nextest's JUnit report, prints the table
and saves it to <target>/check-timings/last.txt, so two runs compare with diff.

Python 3.9 (the one macOS ships): standard library only.
"""

import argparse
import datetime
import os
import subprocess
import sys
import xml.etree.ElementTree as ET


def clock(seconds):
    return "%d:%02d" % (seconds // 60, seconds % 60)


def read_steps(path):
    """Steps in the order they ran, each with its substeps: [(seconds, failed, title, [(seconds, title)])]."""
    steps, pending = [], []
    if not os.path.exists(path):
        return steps
    with open(path, encoding="utf-8") as lines:
        for line in lines:
            parts = line.rstrip("\n").split("\t", 2)
            if len(parts) != 3:
                continue
            seconds, kind, title = int(parts[0]), parts[1], parts[2]
            if kind == "sub":
                pending.append((seconds, title))
            else:
                steps.append((seconds, kind == "failed", title, pending))
                pending = []
    return steps


def slowest_tests(junit, since, count):
    """The slowest test cases in nextest's JUnit report, if this run wrote it."""
    if not junit or not os.path.exists(junit) or os.path.getmtime(junit) < since:
        return []
    cases = []
    for case in ET.parse(junit).getroot().iter("testcase"):
        try:
            seconds = float(case.get("time", "0"))
        except ValueError:
            continue
        cases.append((seconds, "%s %s" % (case.get("classname", ""), case.get("name", ""))))
    cases.sort(key=lambda case: -case[0])
    return cases[:count]


def cargo_reports(folder, since):
    """The cargo-timing-<time>.html reports this run wrote (cargo-timing.html is only the last one)."""
    if not folder or not os.path.isdir(folder):
        return []
    found = []
    for name in sorted(os.listdir(folder)):
        path = os.path.join(folder, name)
        if name.startswith("cargo-timing-") and name.endswith(".html") and os.path.getmtime(path) >= since:
            found.append(path)
    return found


def branch():
    try:
        return subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"], capture_output=True, text=True, check=True
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "?"


def report(args):
    steps = read_steps(args.steps)
    total = args.total
    out = []
    state = "failed" if args.status != 0 else "passed"
    out.append(
        "Time per step (%s, build directory %s, %s, %s, %s):"
        % (args.mode, args.build, state, branch(), datetime.datetime.now().strftime("%Y-%m-%d %H:%M"))
    )
    for seconds, failed, title, subs in sorted(steps, key=lambda step: -step[0]):
        share = 100 * seconds // total if total else 0
        mark = "  FAILED" if failed else ""
        out.append("  %6s %3d%%  %s%s" % (clock(seconds), share, title, mark))
        for sub_seconds, sub_title in subs:
            out.append("                %6s  %s" % (clock(sub_seconds), sub_title))
    counted = sum(step[0] for step in steps)
    if total > counted:
        out.append("  %6s %3d%%  (outside any step: setup and reporting)" % (clock(total - counted), 100 * (total - counted) // total))
    out.append("  %6s  total" % clock(total))

    tests = slowest_tests(args.junit, args.since, 10)
    if tests:
        out.append("")
        out.append("Slowest tests (nextest):")
        for seconds, name in tests:
            out.append("  %7.1fs  %s" % (seconds, name))

    reports = cargo_reports(args.cargo_timings, args.since)
    if reports:
        out.append("")
        out.append("Compile time per crate (cargo --timings), one report per cargo call:")
        out.extend("  %s" % report for report in reports)

    text = "\n".join(out)
    print("\n" + text)
    with open(args.save, "w", encoding="utf-8") as saved:
        saved.write(text + "\n")
    print("(saved to %s)" % args.save)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--steps", required=True, help="the TSV check.sh wrote")
    parser.add_argument("--save", required=True, help="where to save the table")
    parser.add_argument("--total", type=int, required=True, help="seconds since the check started")
    parser.add_argument("--since", type=float, default=0, help="epoch when the check started")
    parser.add_argument("--status", type=int, default=0, help="the check's exit status")
    parser.add_argument("--mode", default="quick")
    parser.add_argument("--build", default="reused", help="empty or reused: the build directory at start")
    parser.add_argument("--junit", help="nextest's JUnit report")
    parser.add_argument("--cargo-timings", help="the folder of cargo's --timings reports, when asked for")
    report(parser.parse_args())


if __name__ == "__main__":
    sys.exit(main())
