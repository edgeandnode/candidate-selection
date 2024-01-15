use candidate_selection::{criteria::performance::Performance, Normalized};
use indexer_selection::Candidate;
use rand::{rngs::SmallRng, Rng, SeedableRng};
use std::io::stdin;
use thegraph::types::Address;

pub struct IndexerCharacteristics {
    pub address: Address,
    pub fee: f64,
    pub seconds_behind: u16,
    pub latency_ms: u32,
    pub success_rate: Normalized,
    pub slashable_usd: u64,
    pub zero_allocation: bool,
}

fn main() {
    let header =
        "address,fee_grt,seconds_behind,latency_ms,success_rate,slashable_stake_grt,allocation_grt";
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
                fee: fields[1].parse().expect("fee"),
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
    let budget = 20e-6;
    let candidates: Vec<Candidate> = characteristics
        .into_iter()
        .map(|c| {
            let mut performance = Performance::new();
            for _ in 0..1000 {
                performance.feedback(rng.gen_bool(c.success_rate.as_f64()), c.latency_ms);
            }
            Candidate {
                indexer: c.address,
                deployment: [0; 32].into(),
                fee: Normalized::new(c.fee / budget).expect("invalid fee or budget"),
                subgraph_versions_behind: 0,
                seconds_behind: c.seconds_behind,
                slashable_usd: c.slashable_usd,
                zero_allocation: c.zero_allocation,
                performance: Box::leak(Box::new(performance)),
            }
        })
        .collect();

    todo!();
}
