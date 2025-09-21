#!/usr/bin/env python3
"""
A tiny 1 kHz CSV writer to demonstrate the csv_tail example.

It appends to a CSV file with the following columns:
index,timestamp_micros,sine,cosine

- Runs until interrupted (Ctrl+C)
- Starts a new file and writes a header if the file does not exist
- Appends if the file exists
"""
from __future__ import annotations
import math
import os
import sys
import time
from datetime import datetime, timezone

FS_HZ = 1000.0  # 1 kHz
DT_S = 1.0 / FS_HZ
F_HZ = 3.0


def now_us() -> int:
    return int(datetime.now(tz=timezone.utc).timestamp() * 1_000_000)


def main() -> int:
    path = sys.argv[1] if len(sys.argv) > 1 else "live_data.csv"

    # Create file and write header if it doesn't exist
    new_file = not os.path.exists(path)
    # Open in text mode, UTF-8, with line buffering for timely writes
    f = open(path, "a", buffering=1, encoding="utf-8", newline="\n")
    try:
        if new_file:
            f.write("index,timestamp_micros,sine,cosine\n")

        n = 0
        next_t = time.perf_counter()
        print(f"[csv_writer] Writing to {path} at ~1 kHz. Press Ctrl+C to stop.")
        while True:
            t = n / FS_HZ
            s = math.sin(2.0 * math.pi * F_HZ * t)
            c = math.cos(2.0 * math.pi * F_HZ * t)
            ts = now_us()
            f.write(f"{n},{ts},{s:.9f},{c:.9f}\n")
            f.flush()
            n = (n + 1) & 0xFFFFFFFFFFFFFFFF

            next_t += DT_S
            # Busy wait a bit to achieve 1 kHz with decent accuracy
            while True:
                now = time.perf_counter()
                if now >= next_t:
                    break
                # Sleep in short bursts to reduce CPU but keep responsiveness
                time.sleep(0.0002)
    except KeyboardInterrupt:
        print("\n[csv_writer] Stopped.")
    finally:
        f.flush()
        f.close()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
