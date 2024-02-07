use candidate_selection::{
    criteria::performance::Performance, num::assert_within, ArrayVec, Normalized,
};
use indexer_selection::{select, Candidate};
use rand::{rngs::SmallRng, Rng, SeedableRng};
use std::{collections::BTreeMap, io::stdin, time::Instant};
use thegraph::types::Address;

struct IndexerCharacteristics {
    address: Address,
    success_rate: Normalized,
    latency_ms: u32,
    fee_usd: f64,
    seconds_behind: u32,
    slashable_usd: u64,
    zero_allocation: bool,
}

fn main() {
    let header =
        "address,fee_usd,seconds_behind,latency_ms,success_rate,slashable_usd,zero_allocation";
    let characteristics: Vec<IndexerCharacteristics> = stdin()
        .lines()
        .filter_map(|line| {
            let line = line.unwrap();
            if line.starts_with(header) {
                return None;
            }
            let fields = line.split(',').collect::<Vec<&str>>();
            Some(IndexerCharacteristics {
                address: fields[0].parse().expect("address"),
                fee_usd: fields[1].parse().expect("fee_usd"),
                seconds_behind: fields[2].parse().expect("seconds_behind"),
                latency_ms: fields[3].parse().expect("latency_ms"),
                success_rate: fields[4]
                    .parse::<f64>()
                    .ok()
                    .and_then(Normalized::new)
                    .expect("success_rate"),
                slashable_usd: fields[5].parse().expect("slashable_usd"),
                zero_allocation: fields[6].parse().expect("zero_allocation"),
            })
        })
        .collect();

    let mut rng = SmallRng::from_entropy();

    let mut perf: BTreeMap<Address, Performance> = characteristics
        .iter()
        .map(|c| {
            let mut perf = Performance::default();
            for _ in 0..10000 {
                perf.feedback(rng.gen_bool(c.success_rate.as_f64()), c.latency_ms);
            }
            let expected = perf.expected_performance();
            assert_within(expected.latency_ms() as f64, c.latency_ms as f64, 1.0);
            assert_within(
                expected.success_rate.as_f64(),
                c.success_rate.as_f64(),
                0.01,
            );
            (c.address, perf)
        })
        .collect();

    let total_client_queries = 10_000;
    let mut total_selection_μs = 0;
    let mut total_latency_ms: u64 = 0;
    let mut total_successes: u64 = 0;
    let mut total_seconds_behind: u64 = 0;
    let mut total_fees_usd = 0.0;

    let budget = 20e-6;
    let client_queries_per_second = 100;
    for client_query_index in 0..total_client_queries {
        if (client_query_index % client_queries_per_second) == 0 {
            for p in perf.values_mut() {
                p.decay();
            }
        }

        let candidates: Vec<Candidate> = characteristics
            .iter()
            .map(|c| Candidate {
                indexer: c.address,
                deployment: [0; 32].into(),
                url: "https://example.com".parse().unwrap(),
                perf: perf.get(&c.address).unwrap().expected_performance(),
                fee: Normalized::new(c.fee_usd / budget).expect("invalid fee or budget"),
                seconds_behind: c.seconds_behind,
                slashable_usd: c.slashable_usd,
                subgraph_versions_behind: 0,
                zero_allocation: c.zero_allocation,
            })
            .collect();

        let t0 = Instant::now();
        let selections: ArrayVec<&Candidate, 3> = select(&mut rng, &candidates);
        total_selection_μs += Instant::now().duration_since(t0).as_micros();
        total_fees_usd += selections
            .iter()
            .map(|c| c.fee.as_f64() * budget)
            .sum::<f64>();

        struct IndexerOutcome {
            indexer: Address,
            latency_ms: u32,
            success: bool,
            seconds_behind: u32,
        }
        let mut indexer_query_outcomes: ArrayVec<IndexerOutcome, 3> = selections
            .iter()
            .map(|c| IndexerOutcome {
                indexer: c.indexer,
                latency_ms: c.perf.latency_ms(),
                success: rng.gen_bool(c.perf.success_rate.as_f64()),
                seconds_behind: c.seconds_behind,
            })
            .collect();
        indexer_query_outcomes.sort_unstable_by_key(|o| o.success.then_some(o.latency_ms));
        let client_outcome = indexer_query_outcomes.iter().find(|o| o.success);

        total_successes += client_outcome.is_some() as u64;
        total_latency_ms += client_outcome.map(|o| o.latency_ms).unwrap_or_else(|| {
            selections
                .iter()
                .map(|c| c.perf.latency_ms())
                .max()
                .unwrap_or(0)
        }) as u64;
        total_seconds_behind += client_outcome.map(|o| o.seconds_behind).unwrap_or(0) as u64;

        drop(selections);
        for outcome in indexer_query_outcomes {
            perf.get_mut(&outcome.indexer)
                .unwrap()
                .feedback(outcome.success, outcome.latency_ms);
        }
    }

    println!(
        "avg_selection_μs: {}",
        total_selection_μs as f64 / total_client_queries as f64
    );
    println!(
        "success_rate: {:.4}",
        total_successes as f64 / total_client_queries as f64
    );
    println!(
        "avg_latency_ms: {:.2}",
        total_latency_ms as f64 / total_client_queries as f64
    );
    println!(
        "total_seconds_behind: {:.2}",
        total_seconds_behind as f64 / total_client_queries as f64
    );
    println!("total_fees_usd: {:.2}", total_fees_usd);
}
