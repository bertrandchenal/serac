#!/usr/bin/env python3

import argparse
import random
from datetime import datetime, timedelta, timezone


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate TSV with timestamp/value columns."
    )
    parser.add_argument(
        "--output",
        "-o",
        default="generated.tsv",
        help="Output TSV path (default: generated.tsv)",
    )
    parser.add_argument(
        "--rows",
        "-n",
        type=int,
        default=50_000,
        help="Number of data rows (default: 50000)",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=None,
        help="Optional random seed for reproducible output",
    )
    args = parser.parse_args()

    if args.rows <= 0:
        raise ValueError("--rows must be > 0")

    rng = random.Random(args.seed)
    start = datetime(2026, 1, 1, 0, 0, 0, tzinfo=timezone.utc)

    with open(args.output, "w", encoding="utf-8", newline="") as file:
        file.write("timestamp\tvalue\n")
        for index in range(args.rows):
            timestamp = (start + timedelta(seconds=index)).strftime(
                "%Y-%m-%dT%H:%M:%SZ"
            )
            value = rng.uniform(10.0, 20.0)
            file.write(f"{timestamp}\t{value:.6f}\n")


if __name__ == "__main__":
    main()

