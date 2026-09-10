//! P3.0-F01 performance budget (spec §8): in-memory fuzzy matching over a
//! 10k-candidate catalog must stay under P95 5ms. This test asserts a
//! generous CI-safe ceiling and prints the measured number for the batch
//! record.

use launcher_search::fuzzy::fuzzy_score_best;

fn catalog(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            let kind = i % 4;
            match kind {
                0 => format!("Application Number {i}"),
                1 => format!("report-quarterly-{i}.txt"),
                2 => format!("设置项 {i}"),
                _ => format!("Settings Toggle {i}"),
            }
        })
        .collect()
}

#[test]
fn fuzzy_p95_budget_at_10k() {
    let cat = catalog(10_000);
    let queries = ["chr", "set", "设置", "report quarterly", "app 42", "wj", "xyzq"];
    let mut samples: Vec<u128> = Vec::new();
    for q in queries {
        for _ in 0..20 {
            let t = std::time::Instant::now();
            let mut best: Option<f32> = None;
            for c in &cat {
                let extra = ["settings", "index"]; // simulate keyword fields
                let s = fuzzy_score_best(
                    [c.as_str(), extra[0], extra[1]],
                    q,
                );
                best = match (best, s) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (None, s) => s,
                    (a, None) => a,
                };
            }
            let _ = best; // None is legal (no candidate matches)
            samples.push(t.elapsed().as_micros());
        }
    }
    samples.sort_unstable();
    let p95_us = samples[samples.len() * 95 / 100];
    println!("fuzzy scan P95 over 10k candidates: {} µs", p95_us);
    assert!(
        p95_us < 20_000,
        "CI-safe ceiling 20ms exceeded: {p95_us}µs (budget is 5ms on release builds)"
    );
}
