use alloc::string::String;
use clap::{arg, Arg, Command};

pub(super) fn cli() -> Command {
    Command::new("DRP_TAG_ADDER")
        .about("Adds a the DRP tag to all instances")
        .arg(
            Arg::new("profile")
                .long("profile")
                .value_parser(clap::value_parser!(String))
                .required(true)
        )
        .arg(
            Arg::new("drp-tier")
                .long("drp-tier")
                .value_parser(["Gold", "Silver", "Bronze"])
                .num_args(1)
                .default_value("Bronze")
        )
}