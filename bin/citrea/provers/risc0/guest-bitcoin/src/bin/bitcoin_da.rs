#![no_main]
use bitcoin_da::spec::RollupParams;
use bitcoin_da::verifier::BitcoinVerifier;

use citrea_stf::runtime::Runtime;
use citrea_stf::StfVerifier;
#[cfg(feature = "bench")]
use risc0_zkvm::guest::env;
use rollup_constants::{DA_TX_ID_LEADING_ZEROS, ROLLUP_NAME};
use sov_modules_api::default_context::ZkDefaultContext;
use sov_modules_stf_blueprint::StfBlueprint;
use sov_risc0_adapter::guest::Risc0Guest;
use sov_rollup_interface::da::DaVerifier;
use sov_state::ZkStorage;

#[cfg(feature = "bench")]
fn report_bench_metrics(start_cycles: usize, end_cycles: usize) {
    let cycles_per_block = (end_cycles - start_cycles) as u64;
    let tuple = ("Cycles per block".to_string(), cycles_per_block);
    let mut serialized = Vec::new();
    serialized.extend(tuple.0.as_bytes());
    serialized.push(0);
    let size_bytes = tuple.1.to_ne_bytes();
    serialized.extend(&size_bytes);

    // calculate the syscall name.
    let cycle_string = String::from("cycle_metrics\0");
    let metrics_syscall_name =
        risc0_zkvm_platform::syscall::SyscallName::from_bytes_with_nul(cycle_string.as_ptr());

    risc0_zkvm::guest::env::send_recv_slice::<u8, u8>(metrics_syscall_name, &serialized);
}

risc0_zkvm::guest::entry!(main);

pub fn main() {
    let guest = Risc0Guest::new();
    let storage = ZkStorage::new();
    #[cfg(feature = "bench")]
    let start_cycles = env::cycle_count();

    let stf: StfBlueprint<ZkDefaultContext, _, _, Runtime<_, _>> = StfBlueprint::new();

    let stf_verifier = StfVerifier::new(
        stf,
        BitcoinVerifier::new(RollupParams {
            rollup_name: ROLLUP_NAME.to_string(),
            reveal_tx_id_prefix: DA_TX_ID_LEADING_ZEROS.to_vec(),
        }),
    );

    stf_verifier
        .run_sequencer_commitments_in_da_slot(guest, storage)
        .expect("Prover must be honest");

    #[cfg(feature = "bench")]
    {
        let end_cycles = env::cycle_count();
        report_bench_metrics(start_cycles, end_cycles);
    }
}
