use clap::Parser;
use std::process;

use taskai::cli::commands::{Cli, Commands};
use taskai::cli;

fn main() {
    let cli_args = Cli::parse();
    let json_output = cli_args.json;
    let plan_flag = cli_args.plan.clone();

    let exit_code = match cli_args.command {
        Commands::Init => cli::init::run(json_output),
        Commands::Plan(cmd) => cli::plan::run(cmd, json_output),
        Commands::Task(cmd) => cli::task::run(cmd, json_output, plan_flag.as_deref()),
        Commands::Next { claim, agent } => {
            cli::next::run(claim, agent.as_deref(), json_output, plan_flag.as_deref())
        }
        Commands::Status => cli::status::run(json_output, plan_flag.as_deref()),
    };

    process::exit(exit_code);
}
