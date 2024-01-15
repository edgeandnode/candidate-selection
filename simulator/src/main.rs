use candidate_selection::Normalized;
use std::io::stdin;
use thegraph::types::Address;

pub struct IndexerCharacteristics {
    pub address: Address,
    pub fee: f64,
    pub avg_seconds_behind: u16,
    pub avg_latency_ms: u32,
    pub success_rate: Normalized,
    pub slashable_stake_grt: u128,
    pub allocation_grt: u128,
}

fn main() {
    let header = "address,fee_grt,avg_seconds_behind,avg_latency_ms,success_rate,slashable_stake_grt,allocation_grt";
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
                avg_seconds_behind: fields[2].parse().expect("avg_seconds_behind"),
                avg_latency_ms: fields[3].parse().expect("avg_latency_ms"),
                success_rate: fields[4]
                    .parse::<f64>()
                    .ok()
                    .and_then(Normalized::new)
                    .expect("success_rate"),
                slashable_stake_grt: fields[5].parse().expect("slashable_stake_grt"),
                allocation_grt: fields[6].parse().expect("allocation_grt"),
            })
        })
        .collect();

    todo!();
}
