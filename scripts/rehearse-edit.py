"""Rehearse the one-minute edit (ROADMAP step 2's bar) through the engine, with the Sony and
Samsung clips: a new timeline, ranges marked in the source viewer, Insert, Overwrite, Append at
End, Place on Top, a split and a ripple delete, a ripple trim, a roll, a dissolve, a marker, undo
and redo, then Deliver to H.264, checked with ffprobe (LOG B-042).

    python scripts/rehearse-edit.py <out dir> [<media dir>]

Run from the repo after `cargo build -p resolution-cli`. It works step by step through the CLI,
the project saved between steps, and reports each step ok or with the engine's error, so one
failure does not hide the next. The clips are local (`test media/` is not in git).
"""
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CLI = os.path.join(ROOT, "target", "debug", "resolution.exe" if os.name == "nt" else "resolution")
MEDIA = sys.argv[2] if len(sys.argv) > 2 else os.path.join(ROOT, "test media", "sony-samsung-2026-10-12")
SONY, SAMSUNG = os.path.join(MEDIA, "ROB MID SHOT.MP4"), os.path.join(MEDIA, "NATAL SAD.mp4")
OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)
PROJECT = f"{OUT}/rehearsal.fcproj"
results = []


def run(name, cmds, first=False):
    """Runs `cmds` ([(id, params)]) on the saved project and saves it again."""
    args = [CLI, "run"]
    if not first:
        args += ["--project", PROJECT]
    for cid, params in cmds:
        args += [cid, json.dumps(params)]
    args += ["project.saveAs", json.dumps({"path": PROJECT})]
    r = subprocess.run(args, capture_output=True, text=True)
    outs = [json.loads(l) for l in r.stdout.splitlines() if l.strip().startswith("{")]
    ok = r.returncode == 0
    err = r.stderr.strip().splitlines()[-1] if not ok and r.stderr.strip() else ""
    results.append((name, ok, err))
    print(f"{'ok ' if ok else 'ERR'} {name}" + (f": {err}" if err else ""))
    return outs


def inspect():
    outs = run("(inspect)", [("timeline.inspect", {})])
    for o in outs:
        if o["command"] == "timeline.inspect":
            return o["result"]
    return None


def clips(tl, kind="video", track=0):
    tracks = tl.get(kind, [])
    if track >= len(tracks):
        return []
    ticks = 254016000000
    return [(c["clip"], c.get("name"), round(c.get("start", 0) / ticks, 3), round(c.get("duration", 0) / ticks, 3))
            for c in tracks[track].get("items", [])]


# 1. the media, and a new timeline at the Sony's rate
outs = run("import both clips, new timeline", [
    ("media.import", {"paths": [SONY]}),
    ("media.import", {"paths": [SAMSUNG]}),
    ("timeline.new", {"name": "Rehearsal", "fps": 23.976}),
], first=True)
items = [o["result"]["items"][0] for o in outs if o["command"] == "media.import"]
sony, samsung = items
print("items", items)


def source(item, a, b):
    """Opens `item` in the source viewer, marked from frame a to frame b."""
    return [("source.open", {"item": item}), ("source.seek", {"frame": a}), ("mark.in", {"target": "source"}),
            ("source.seek", {"frame": b}), ("mark.out", {"target": "source"})]


# 2. the cut, as an editor builds it
run("F9: insert 10 s of Sony at the start", [("playhead.set", {"frame": 0})] + source(sony, 48, 48 + 239) + [("edit.insert", {})])
run("F10: overwrite 8 s of Samsung after it", source(samsung, 30, 30 + 239) + [("edit.overwrite", {})])
run("Shift+F12: append 12 s more Sony", source(sony, 400, 400 + 287) + [("edit.appendAtEnd", {})])
run("F12: a 2.4 s Samsung cutaway on top at 5 s", [("playhead.set", {"seconds": 5.0})] + source(samsung, 300, 300 + 71) + [("edit.placeOnTop", {})])
run("F9 at 20 s: insert 6 s of Samsung mid-timeline", [("playhead.set", {"seconds": 20.0})] + source(samsung, 500, 500 + 143) + [("edit.insert", {})])
tl = inspect()
print("V1", clips(tl), "\nV2", clips(tl, track=1))

# 3. tightening: split and ripple delete a second, trim an edge, roll, a transition
run("split at 15 s and at 16 s", [("playhead.set", {"seconds": 15.0}), ("timeline.split", {}),
                                   ("playhead.set", {"seconds": 16.0}), ("timeline.split", {})])
tl = inspect()
v1 = clips(tl)
mid = [c for c in v1 if c[2] is not None]
print("V1 after splits", v1)
piece = next((c for c in v1 if abs(c[2] - 15.0) < 0.1), None)
print("piece", piece)
if piece:
    run("ripple delete the one-second piece", [("timeline.rippleDelete", {"clips": [piece[0]]})])
tl = inspect()
v1 = clips(tl)
if len(v1) > 1:
    run("trim the first clip's end back 12 frames (ripple)", [("timeline.trimEdge", {"clip": v1[0][0], "edge": "out", "mode": "ripple", "deltaFrames": -12})])
    tl = inspect()
    v1 = clips(tl)
    run("roll the edit between clips 2 and 3 by 6 frames", [("timeline.rollEdit", {"left": v1[1][0], "right": v1[2][0], "deltaFrames": 6})])
    run("a cross dissolve on clip 1's out", [("timeline.addVideoTransition", {"clip": v1[0][0], "edge": "out", "frames": 12})])
run("a marker at 30 s", [("playhead.set", {"seconds": 30.0}), ("marker.add", {"name": "music hit"})])
run("undo, then redo", [("edit.undo", {}), ("edit.redo", {})])
tl = inspect()
print("final V1", clips(tl), "\nfinal V2", clips(tl, track=1), "\nA1", clips(tl, "audio", 0))

# 4. Deliver, and the file checked
mp4 = f"{OUT}/rehearsal.mp4"
if os.path.exists(mp4):
    os.remove(mp4)
outs = run("Deliver the whole timeline, H.264 1080p", [("deliver.render", {"path": mp4, "preset": "High Quality 1080p HD", "wait": True})])
for o in outs:
    if o["command"] == "deliver.render":
        print("render", json.dumps(o["result"].get("result", o["result"]))[:300])
if os.path.exists(mp4):
    p = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration:stream=codec_type,codec_name,width,height,r_frame_rate",
                        "-of", "json", mp4], capture_output=True, text=True)
    print("ffprobe", p.stdout.replace("\n", " ")[:600])

print("\nSUMMARY")
for name, ok, err in results:
    if not ok:
        print(f"FAILED: {name}: {err}")
print(f"{sum(1 for r in results if r[1])} of {len(results)} steps ok")
