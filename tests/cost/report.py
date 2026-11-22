"""Paired trial medians and upper one-sided percentile bootstrap confidence bounds."""
import csv
import json
import random
import statistics
import sys
from pathlib import Path

root = Path(sys.argv[1])
conditions = ["may", "inactive", "active"]
data = {}
for name in conditions:
    trials = sorted(root.glob(name+"-*.csv"), key=lambda p: int(p.stem.split("-")[-1]))
    assert len(trials) >= 10
    data[name] = []
    for trial in trials:
        ops = {}
        with trial.open() as stream:
            for op, cpu, process, alloc in csv.reader(stream):
                ops.setdefault(op, []).append((int(cpu)/1000, int(process)/1000, int(alloc)))
        assert len(ops) == 4 and all(len(rows) == 1000 for rows in ops.values())
        data[name].append({op: {"median_us": statistics.median(r[0] for r in rows),
                "p95_us": sorted(r[0] for r in rows)[949],
                "allocations": statistics.mean(r[2] for r in rows),
                "other_cpu_us": statistics.mean(r[1]-r[0] for r in rows)} for op, rows in ops.items()})
assert len(set(map(len, data.values()))) == 1
rng = random.Random(56065)
def upper95(values):
    draws = sorted(statistics.mean(rng.choices(values, k=len(values))) for _ in range(10000))
    return draws[9499]

summary = {name: {op: {metric: round(statistics.median(t[op][metric] for t in trials),3)
                       for metric in trials[0][op]} for op in trials[0]} for name,trials in data.items()}
summary["trials"] = len(data["may"])
summary["inactive_upper95_percent"] = {op: round(upper95([100*(i[op]["median_us"]/m[op]["median_us"]-1)
        for i,m in zip(data["inactive"],data["may"])]),3) for op in data["may"][0]}
summary["edit_upper95_added_p95_us"] = round(upper95([a["edit"]["p95_us"]-i["edit"]["p95_us"]
        for a,i in zip(data["active"],data["inactive"])]),3)
sys.dont_write_bytecode = True
from traffic import traffic
summary["trace"] = {key: traffic(root, key) for key in ("active", "inactive")}
inactive = summary["trace"]["inactive"]
assert all(row["updates"] == row["semantic_allocations"] == 0 for row in inactive.values()), inactive
activation = sorted(int(path.read_text())/1000 for path in root.glob("activation-*.ns"))
assert len(activation) >= 10
summary["activation_us"] = dict(trials=len(activation), median=round(statistics.median(activation),3),
                                p95=activation[(95*len(activation)+99)//100-1])
cached = summary["trace"]["active"]["cached"]
samples = [json.loads(p.read_text()) for p in root.glob("trace-active.csv.nodes-*.json")]
assert any(sum(n["role"] == "listBoxOption" for _, n in sample["nodes"]) == 20 for sample in samples)
# The pinned producer sends a complete tree every time it submits an update.
# Check the child references on its root-only payload as well as D-Bus traffic.
lost_rows = any(len(s["nodes"]) == 1 and not s["nodes"][0][1]["properties"].get("children") for s in samples)
summary["cached_semantics_pass"] = cached["bus_signals"] == 0 and not (cached["nodes"] == cached["updates"] and lost_rows)
print(json.dumps(summary, indent=2))
sys.exit(int(not summary["cached_semantics_pass"] or max(summary["inactive_upper95_percent"].values()) >= 1 or summary["edit_upper95_added_p95_us"] >= 500))
