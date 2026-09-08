# PERFORMANCE.md

## Budgets

| Metric | Target | Hard Gate |
|---|---:|---:|
| Idle Private Bytes | <= 50 MB | <= 80 MB |
| Idle CPU | < 0.1% | define per test environment |
| Hotkey -> visible UI P50 | <= 20 ms | <= 35 ms P95 |
| App search P50 | <= 5 ms | <= 10 ms |
| File search P50 | <= 20 ms | <= 50 ms |
| IPC P50 | <= 1 ms | <= 5 ms |

These are project budgets, not claims about competitor measurements.

## Measure

At minimum:

- Private Bytes
- Working Set
- Commit
- CPU time
- process count
- thread count
- handle count
- startup latency
- hotkey latency
- search latency
- IPC latency

## Benchmark Rules

1. Warm and cold runs must be separated.
2. Benchmark environment must be recorded.
3. Compare against baseline.
4. >5% regression = warning.
5. >10% regression = fail unless baseline is explicitly updated with an ADR/benchmark note.
