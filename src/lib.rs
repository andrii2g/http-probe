pub mod cli;
pub mod error;
pub mod model;
pub mod output;
pub mod probe;
pub mod stats;

use std::thread::sleep;

use clap::Parser;
use cli::Cli;
use error::AppError;
use model::{AttemptResult, RunSummary};

pub fn run_from_env() -> Result<u8, AppError> {
    let cli = Cli::parse();
    let summary = execute(cli)?;
    output::print_report(&summary);
    Ok(exit_code_for_summary(&summary))
}

pub fn execute(cli: Cli) -> Result<RunSummary, AppError> {
    let config = cli.validate()?;
    let mut attempts = Vec::with_capacity(config.count as usize);

    for attempt_number in 1..=config.count {
        let result = probe::probe_once(&config, attempt_number)?;
        if config.verbose {
            output::print_verbose_attempt(&result, config.count);
        }
        attempts.push(result);

        if attempt_number != config.count && !config.interval.is_zero() {
            sleep(config.interval);
        }
    }

    Ok(stats::build_run_summary(&config, attempts))
}

pub fn exit_code_for_summary(summary: &RunSummary) -> u8 {
    if summary
        .attempts
        .iter()
        .all(|attempt| matches!(attempt, AttemptResult::Success(_)))
    {
        0
    } else {
        1
    }
}
