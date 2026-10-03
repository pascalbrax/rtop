"""Live Linux comparisons. Run after cargo build --release, in the same namespace as tools."""
import json
import os
from pathlib import Path
import subprocess
import time

BIN = "target/release/rtop"

def run(*args):
    return subprocess.check_output(args, text=True)

def free():
    row = next(line for line in run("free", "-b").splitlines() if line.startswith("Mem:"))
    values = list(map(int, row.split()[1:]))
    return values[0], values[-1]

def network():
    records = json.loads(run("ip", "-j", "-s", "link", "show"))
    return {r["ifname"]: (r.get("stats64", r.get("stats"))["rx"]["bytes"],
                            r.get("stats64", r.get("stats"))["tx"]["bytes"]) for r in records}

def disk():
    result = {}
    for device in Path("/sys/dev/block").iterdir():
        try:
            fields = list(map(int, (device/"stat").read_text().split()))
            result[device.name] = (fields[2]*512, fields[6]*512)
        except (OSError, ValueError):
            pass
    return result

before_free = free()
before_net = network()
before_disk = disk()
vmstat = subprocess.Popen(["vmstat", "-n", "1", "3"], stdout=subprocess.PIPE, text=True)
output = run(BIN, "--collect", "3", "--interval", "1000")
after_free = free()
after_net = network()
after_disk = disk()
vmstat_output = vmstat.communicate(timeout=5)[0]
assert vmstat.returncode == 0
samples = []
for line in output.splitlines():
    fields = line.split("\t")
    if fields[0] == "sample":
        samples.append({"time": float(fields[2]), "rows": []})
    elif fields[0] != "#" and not line.startswith("#"):
        samples[-1]["rows"].append(fields)
assert len(samples) == 3
assert not [r for s in samples for r in s["rows"] if r[0] == "error"]
first = samples[0]["rows"]
assert all(r[2] == "sampling" for r in first if r[0] == "cpu")
checked_net = checked_disks = 0
for sample in samples:
    for r in sample["rows"]:
        if r[0] == "memory":
            assert int(r[1]) == before_free[0] == after_free[0]
            assert abs(int(r[2])-before_free[1]) < before_free[0]*0.01
            assert abs(int(r[2])-after_free[1]) < after_free[0]*0.01
            assert int(r[1])-int(r[2]) == int(r[3])
        if r[0] == "network" and r[1] in before_net and r[1] in after_net:
            for i in range(2):
                assert before_net[r[1]][i] <= int(r[3+i]) <= after_net[r[1]][i]
            checked_net += 1
        if r[0] == "disk" and r[1] in before_disk and r[1] in after_disk:
            for i in range(2):
                assert before_disk[r[1]][i] <= int(r[4+i]) <= after_disk[r[1]][i]
            checked_disks += 1
        if r[0] == "filesystem" and r[2] == "/":
            df = run("df", "-B1", "--output=size,used,avail", "/").splitlines()[1].split()
            assert int(df[0]) == int(r[4])
            assert abs(int(df[1])-int(r[5])) < int(r[4])*0.001
            assert abs(int(df[2])-int(r[7])) < int(r[4])*0.001
assert checked_net > 0 and checked_disks > 0
# Rates must agree with exported counters and elapsed monotonic time.
for previous, current in zip(samples, samples[1:]):
    old = {(r[0],r[1]): r for r in previous["rows"] if r[0] in ("network","disk")}
    dt = current["time"]-previous["time"]
    assert 0.9 < dt < 1.2
    for r in current["rows"]:
        if (r[0],r[1]) not in old:
            continue
        b = old[(r[0],r[1])]
        positions = (3,4,5,6) if r[0] == "network" else (4,5,6,7)
        for counter, rate in zip(positions[:2], positions[2:]):
            expected = (int(r[counter])-int(b[counter]))/dt
            actual = float(r[rate])
            assert abs(expected-actual) <= max(1,abs(expected)*0.01)
# vmstat prints since-boot averages first; only subsequent interval rows are comparable.
vm_rows = [line.split() for line in vmstat_output.splitlines() if line.split() and line.split()[0].isdigit()]
reported = [float(next(r[2] for r in s["rows"] if r[:2] == ["cpu","cpu"])) for s in samples[1:]]
reference = [100-float(row[14])-float(row[15]) for row in vm_rows[1:]]
assert len(reference) == 2
assert abs(sum(reported)/2-sum(reference)/2) < 5.0
print("PASS: RAM vs free -b; CPU vs vmstat (two near-aligned 1 s intervals, tolerance 5 pp)")
print(f"PASS: network vs ip -j -s ({checked_net} rows); disk vs sysfs ({checked_disks} rows); root space vs df -B1")
print("PASS: rates vs counter deltas (1% tolerance for per-collector timestamp differences)")
print("CPU rtop:", reported, "vmstat:", reference)
print("Namespaces:", os.readlink("/proc/self/ns/net"), os.readlink("/proc/self/ns/mnt"))
Path("docs/benchmarks/live-samples.tsv").write_text(output)
