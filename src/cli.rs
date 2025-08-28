use alloc::string::String;
use clap::{Arg, Command, value_parser};

pub(super) fn cli() -> Command {
    Command::new("DRP_TAG_ADDER")
        .about("Adds the DRP tag to all instances")
        .subcommand_required(true)
        .subcommands([
            Command::new("all")
                .about("Reach all instances")
                .args(tag_arguments()),
            Command::new("instance")
                .about("Modify a single instance")
                .arg(
                    Arg::new("instance_id")
                        .required(true)
                        .value_parser(value_parser!(String)),
                )
                .args(tag_arguments()),
            Command::new("DRP")
                .about("Adds the drp tag to all instances")
                .arg(
                    Arg::new("drp-tier")
                        .required(true)
                        .value_parser(["Gold", "Silver", "Bronze"])
                        .num_args(1)
                        .default_value("Bronze"),
                ),
        ])
        .arg(
            Arg::new("profile")
                .long("profile")
                .value_parser(clap::value_parser!(String))
                .required(true),
        )
}
fn tag_arguments() -> Vec<Arg> {
    vec![
        Arg::new("key")
            .value_name("KEY")
            .value_parser(clap::value_parser!(String))
            .help("The key you want to add")
            .required(true),
        Arg::new("value")
            .value_name("VALUE")
            .value_parser(clap::value_parser!(String))
            .help("The value you want to add")
            .required(true),
    ]
}
