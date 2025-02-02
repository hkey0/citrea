use core::fmt::Debug as DebugTrait;

use anyhow::Context as _;
use bitcoin_da::service::DaServiceConfig;
use citrea::{initialize_logging, BitcoinRollup, MockDemoRollup};
use citrea_sequencer::SequencerConfig;
use citrea_stf::genesis_config::GenesisPaths;
use clap::Parser;
use sov_mock_da::MockDaConfig;
use sov_modules_api::Spec;
use sov_modules_rollup_blueprint::RollupBlueprint;
use sov_state::storage::NativeStorage;
use sov_stf_runner::{from_toml_path, ProverConfig, RollupConfig};
use tracing::{error, instrument};

#[cfg(test)]
mod test_rpc;

/// Main demo runner. Initializes a DA chain, and starts a demo-rollup using the provided.
/// If you're trying to sign or submit transactions to the rollup, the `sov-cli` binary
/// is the one you want. You can run it `cargo run --bin sov-cli`.

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the genesis configuration.
    /// Defines the genesis of module states like evm.
    #[arg(long)]
    genesis_paths: String,

    /// The data layer type.
    #[arg(long, default_value = "mock")]
    da_layer: SupportedDaLayer,

    /// The path to the rollup config.
    #[arg(long, default_value = "configs/mock/rollup_config.toml")]
    rollup_config_path: String,

    /// The path to the sequencer config. If set, runs the node in sequencer mode, otherwise in full node mode.
    #[arg(long, conflicts_with = "prover_config_path")]
    sequencer_config_path: Option<String>,

    /// The path to the prover config. If set, runs the node in prover mode, otherwise in full node mode.
    #[arg(long, conflicts_with = "sequencer_config_path")]
    prover_config_path: Option<String>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum SupportedDaLayer {
    Mock,
    Bitcoin,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    initialize_logging();

    let args = Args::parse();
    let rollup_config_path = args.rollup_config_path.as_str();

    let sequencer_config: Option<SequencerConfig> =
        args.sequencer_config_path.clone().map(|path| {
            from_toml_path(path)
                .context("Failed to read sequencer configuration")
                .unwrap()
        });

    let prover_config: Option<ProverConfig> = args.prover_config_path.clone().map(|path| {
        from_toml_path(path)
            .context("Failed to read prover configuration")
            .unwrap()
    });

    if prover_config.is_some() && sequencer_config.is_some() {
        return Err(anyhow::anyhow!(
            "Cannot run in both prover and sequencer mode at the same time"
        ));
    }

    match args.da_layer {
        SupportedDaLayer::Mock => {
            start_rollup::<MockDemoRollup, MockDaConfig>(
                &GenesisPaths::from_dir(&args.genesis_paths),
                rollup_config_path,
                prover_config,
                sequencer_config,
            )
            .await?;
        }
        SupportedDaLayer::Bitcoin => {
            start_rollup::<BitcoinRollup, DaServiceConfig>(
                &GenesisPaths::from_dir(&args.genesis_paths),
                rollup_config_path,
                prover_config,
                sequencer_config,
            )
            .await?;
        }
    }

    Ok(())
}

#[instrument(level = "trace", skip_all, err)]
async fn start_rollup<S, DaC>(
    rt_genesis_paths: &<<S as RollupBlueprint>::NativeRuntime as sov_modules_stf_blueprint::Runtime<
        <S as RollupBlueprint>::NativeContext,
        <S as RollupBlueprint>::DaSpec,
    >>::GenesisPaths,
    rollup_config_path: &str,
    prover_config: Option<ProverConfig>,
    sequencer_config: Option<SequencerConfig>,
) -> Result<(), anyhow::Error>
where
    DaC: serde::de::DeserializeOwned + DebugTrait + Clone,
    S: RollupBlueprint<DaConfig = DaC>,
    <<S as RollupBlueprint>::NativeContext as Spec>::Storage: NativeStorage,
{
    let rollup_config: RollupConfig<DaC> = from_toml_path(rollup_config_path)
        .context("Failed to read rollup configuration")
        .unwrap();
    let rollup_blueprint = S::new();

    if let Some(sequencer_config) = sequencer_config {
        let sequencer_rollup = rollup_blueprint
            .create_new_sequencer(rt_genesis_paths, rollup_config.clone(), sequencer_config)
            .await
            .unwrap();
        if let Err(e) = sequencer_rollup.run().await {
            error!("Error: {}", e);
        }
    } else if let Some(prover_config) = prover_config {
        let prover = rollup_blueprint
            .create_new_prover(rt_genesis_paths, rollup_config, prover_config)
            .await
            .unwrap();
        if let Err(e) = prover.run().await {
            error!("Error: {}", e);
        }
    } else {
        let rollup = rollup_blueprint
            .create_new_rollup(rt_genesis_paths, rollup_config)
            .await
            .unwrap();
        if let Err(e) = rollup.run().await {
            error!("Error: {}", e);
        }
    }

    Ok(())
}
