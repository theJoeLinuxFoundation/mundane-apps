"""Attribute captured AT-SPI object signals to diagnostic frame intervals."""
import csv
import struct
from pathlib import Path


def packets(pcap):
    with pcap.open("rb") as stream:
        header = stream.read(24)
        endian = {b"\xd4\xc3\xb2\xa1": "<", b"\xa1\xb2\xc3\xd4": ">"}[header[:4]]
        while record := stream.read(16):
            sec, usec, length, _ = struct.unpack(endian+"IIII", record)
            payload = stream.read(length)
            assert len(payload) == length
            if b"org.a11y.atspi.Event.Object\x00" in payload:
                yield sec * 10**9 + usec * 1000, length


def traffic(directory, condition):
    events = list(packets(directory/f"bus-{condition}.pcap"))
    rows = {}
    with (directory/f"trace-{condition}.csv").open() as file:
        for op, *fields in csv.reader(file):
            updates, nodes, payload, activation, start, end, cpu, *extra = map(int, fields)
            sizes = [size for t, size in events if start <= t < end]
            rows[op] = dict(frames=1050, updates=updates, nodes=nodes, json_bytes=payload,
                            first_snapshot_us=round(activation/1000,2), bus_signals=len(sizes),
                            bus_bytes=sum(sizes), adapter_cpu_us_per_frame=round(cpu/1050000,3))
            if extra:
                rows[op]["semantic_allocations"] = extra[0]
    return rows


if __name__ == "__main__":
    import json, sys
    print(json.dumps({v:traffic(Path(sys.argv[1]),v) for v in ("active","inactive")}, indent=2))
