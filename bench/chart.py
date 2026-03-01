#!/usr/bin/env python3
"""
Generate SVG benchmark charts from hyperfine JSON results.

Usage:
    python3 bench/chart.py                    # reads bench/results/*.json
    python3 bench/chart.py /path/to/results   # custom results dir

Outputs SVG files to bench/charts/
"""

import json
import sys
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import matplotlib.ticker as ticker

RESULTS_DIR = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("bench/results")
CHARTS_DIR = Path("bench/charts")
CHARTS_DIR.mkdir(parents=True, exist_ok=True)

# ── Styling ──────────────────────────────────────────────────────
COLOR_ZOXIDE = "#6C8EBF"  # steel blue
COLOR_TP = "#D4A03C"      # warm gold
BG_COLOR = "#0d1117"      # github dark bg
TEXT_COLOR = "#c9d1d9"    # github dark text
GRID_COLOR = "#21262d"    # subtle grid
BAR_HEIGHT = 0.35
FONT = "monospace"

plt.rcParams.update({
    "font.family": FONT,
    "font.size": 11,
    "text.color": TEXT_COLOR,
    "axes.labelcolor": TEXT_COLOR,
    "axes.edgecolor": GRID_COLOR,
    "xtick.color": TEXT_COLOR,
    "ytick.color": TEXT_COLOR,
    "figure.facecolor": BG_COLOR,
    "axes.facecolor": BG_COLOR,
    "savefig.facecolor": BG_COLOR,
    "savefig.edgecolor": BG_COLOR,
})


def load_result(name):
    """Load a hyperfine JSON result file and return (zoxide_ms, tp_ms, zoxide_std, tp_std)."""
    path = RESULTS_DIR / f"{name}.json"
    if not path.exists():
        return None
    with open(path) as f:
        data = json.load(f)

    results = {}
    for r in data["results"]:
        cmd = r["command"]
        mean_ms = r["mean"] * 1000
        std_ms = r["stddev"] * 1000
        # Identify tool by command name
        if "zoxide" in cmd:
            results["zoxide"] = (mean_ms, std_ms)
        else:
            results["tp"] = (mean_ms, std_ms)
    return results


def make_chart(title, groups, filename, subtitle=None):
    """
    Generate a horizontal grouped bar chart.

    groups: list of (label, {tool: (mean_ms, std_ms)}) tuples
    """
    n = len(groups)
    fig_height = max(2.2, 0.55 * n + 1.4)
    fig, ax = plt.subplots(figsize=(8, fig_height))

    labels = [g[0] for g in groups]
    y_positions = list(range(n))

    zoxide_means = []
    zoxide_stds = []
    tp_means = []
    tp_stds = []

    for _, data in groups:
        z = data.get("zoxide", (0, 0))
        t = data.get("tp", (0, 0))
        zoxide_means.append(z[0])
        zoxide_stds.append(z[1])
        tp_means.append(t[0])
        tp_stds.append(t[1])

    # Bars
    y_zoxide = [y + BAR_HEIGHT / 2 for y in y_positions]
    y_tp = [y - BAR_HEIGHT / 2 for y in y_positions]

    bars_z = ax.barh(
        y_zoxide, zoxide_means, BAR_HEIGHT,
        xerr=zoxide_stds, label="zoxide",
        color=COLOR_ZOXIDE, edgecolor="none",
        error_kw={"ecolor": "#8aa8cf", "capsize": 3, "linewidth": 1},
    )
    bars_t = ax.barh(
        y_tp, tp_means, BAR_HEIGHT,
        xerr=tp_stds, label="tp",
        color=COLOR_TP, edgecolor="none",
        error_kw={"ecolor": "#e0be6a", "capsize": 3, "linewidth": 1},
    )

    # Value labels on bars
    for bar, mean in zip(bars_z, zoxide_means):
        ax.text(
            bar.get_width() + 0.3, bar.get_y() + bar.get_height() / 2,
            f"{mean:.1f}ms", va="center", ha="left",
            fontsize=9, color="#8aa8cf",
        )
    for bar, mean in zip(bars_t, tp_means):
        ax.text(
            bar.get_width() + 0.3, bar.get_y() + bar.get_height() / 2,
            f"{mean:.1f}ms", va="center", ha="left",
            fontsize=9, color="#e0be6a",
        )

    ax.set_yticks(y_positions)
    ax.set_yticklabels(labels)
    ax.invert_yaxis()
    ax.set_xlabel("Time (ms) — lower is better")
    ax.xaxis.set_major_formatter(ticker.FormatStrFormatter("%.0f"))
    ax.grid(axis="x", color=GRID_COLOR, linewidth=0.5)
    ax.set_axisbelow(True)

    # Title
    title_text = title
    if subtitle:
        title_text += f"\n{subtitle}"
    ax.set_title(title_text, fontsize=13, fontweight="bold", color=TEXT_COLOR, pad=12)

    ax.legend(
        loc="lower right", frameon=False,
        fontsize=10, labelcolor=TEXT_COLOR,
    )

    # Pad right side for labels
    x_max = max(max(zoxide_means), max(tp_means)) * 1.35
    ax.set_xlim(0, x_max)

    plt.tight_layout()
    out = CHARTS_DIR / filename
    fig.savefig(out, format="svg", bbox_inches="tight")
    plt.close(fig)
    print(f"  {out}")


# ── Generate charts ──────────────────────────────────────────────

print("Generating benchmark charts...")

# Chart 1: Core queries
core_groups = []
for name, label in [("exact", "Exact match"), ("fuzzy", "Broad fuzzy"), ("multi", "Multi-token"), ("add", "Add (write)")]:
    data = load_result(name)
    if data:
        core_groups.append((label, data))

if core_groups:
    make_chart(
        "Core Benchmarks",
        core_groups,
        "core.svg",
        subtitle="500 entries · 1 visit each · M4 Pro",
    )

# Chart 2: Realistic visit patterns
varied_groups = []
for name, label in [("varied-hot", "Hot path (20 visits)"), ("varied-cold", "Cold path (1 visit)"), ("varied-ambiguous", "Ambiguous (300 match)")]:
    data = load_result(name)
    if data:
        varied_groups.append((label, data))

if varied_groups:
    make_chart(
        "Realistic Visit Patterns",
        varied_groups,
        "varied.svg",
        subtitle="300 entries · hot/warm/cold distribution · M4 Pro",
    )

# Chart 3: Stale paths
stale_groups = []
for name, label in [("stale", "Broad (stale + live)"), ("stale-specific", "Specific live path")]:
    data = load_result(name)
    if data:
        stale_groups.append((label, data))

if stale_groups:
    make_chart(
        "Stale Path Handling",
        stale_groups,
        "stale.svg",
        subtitle="200 entries · 40% deleted · M4 Pro",
    )

# Chart 4: Scale
scale_groups = []
for name, label in [("scale-exact", "Exact match"), ("scale-broad", "Broad query")]:
    data = load_result(name)
    if data:
        scale_groups.append((label, data))

if scale_groups:
    make_chart(
        "Scale Test",
        scale_groups,
        "scale.svg",
        subtitle="5,000 entries · M4 Pro",
    )

print("Done.")
