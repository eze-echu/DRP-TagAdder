extern crate alloc;

mod cli;
use cli::cli;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use aws_config::BehaviorVersion;
use aws_config::stalled_stream_protection::StalledStreamProtectionConfig;
use aws_sdk_ec2::operation::RequestId;
use aws_sdk_ec2::types::{Filter, Instance, Tag};
#[cfg(feature = "tracing")]
use log::{error, info, trace};
use std::cmp::Ordering;
use tokio::time;

#[tokio::main]
async fn main() {
    let matches = cli().get_matches();
    let profile = matches.get_one::<String>("profile").unwrap();
    #[cfg(feature = "tracing")]
    tracing_subscriber::fmt::init();
    let timeout_config = aws_config::timeout::TimeoutConfig::builder()
        .connect_timeout(time::Duration::from_secs(10000))
        .operation_timeout(time::Duration::from_secs(10000 * 3))
        .operation_attempt_timeout(time::Duration::from_secs(10000 * 3 * 3))
        .build();
    let config = aws_config::defaults(BehaviorVersion::latest())
        .profile_name(profile)
        .timeout_config(timeout_config)
        .stalled_stream_protection(
            StalledStreamProtectionConfig::enabled()
                .grace_period(time::Duration::from_secs(600))
                .is_enabled(true)
                .build(),
        )
        .load()
        .await;
    let client = aws_sdk_ec2::Client::new(&config);
    match matches.subcommand() {
        Some((subcommand, arg)) => {
            match subcommand {
                "all" => {
                    todo!()
                }
                "instance" => {
                    todo!()
                }
                "drp" => {
                    let drp_tier = matches.get_one::<String>("drp-tier").unwrap();

                    add_tags_to_all_instances(&client, drp_tier).await;

                }
                _ => {
                    eprintln!("Not a valid command")
                }
            }
        }
        None => {eprintln!("No subcommand provided");}
    }
}
async fn add_tag_to_instance(client: &aws_sdk_ec2::Client, tag: &Tag, instance_id: &str) {
    #[cfg(feature = "tracing")]
    info!("about to edit: {:?}", instance_id);
    let a = client
        .create_tags()
        .resources(instance_id)
        .tags(tag.clone())
        .send()
        .await;
    match a {
        Ok(tag_output) => {
            #[cfg(feature = "tracing")]
            info!("{:?}", tag_output);
            println!(
                "Successfully created tag: {:?}",
                tag.key().unwrap_or_default()
            );
            println!(
                "RequestID: {:?}",
                tag_output.request_id().unwrap_or_default()
            );
        }
        Err(e) => {
            #[cfg(feature = "tracing")]
            error!("Error: {:?}", e);
            eprintln!("Error while adding the tag: {:?}", e);
        }
    }
}
async fn add_tags_to_all_instances(client: &aws_sdk_ec2::Client, drp_tier: &str) {
    let instances = match get_all_instances(client).await {
        Some(instances_vec) => instances_vec,
        None => {
            eprintln!("Error: No Instances to edit");
            return;
        }
    };
    // let instance_id_vec = instances.iter().map(|i| i.instance_id().unwrap_or_default()).collect::<Vec<_>>();
    // println!("You are about to edit these instances {:?}", instance_id_vec);
    let tag = Tag::builder().key("DRPBackupPlan").value(drp_tier).build();
    let instances = filter_instances_by_tag_presence(&instances, &tag, false);

    let mut names = get_all_instance_names(&instances).unwrap();
    names.sort();
    println!("About to edit {} instances: {:#?}", instances.len(), names);

    let a = get_all_instance_ids(&instances);
    for instance in instances {
        let Some(instance_id) = instance.instance_id() else {
            continue;
        };
        println!("Adding Tag to instance {}", instance_id);
        println!("Proceed? [y/N]");
        let mut user_response = String::new();
        std::io::stdin().read_line(&mut user_response).unwrap();
        if !user_response.starts_with("y") && !user_response.starts_with("Y") {
            println!("Skipping {}", instance_id);
            continue;
        }
        add_tag_to_instance(client, &tag, &instance_id).await;
    }
    // instance_id_vec.par_iter().for_each(|instance_id| {
    //     let _ = add_tag_to_instance(client, &tag, &instance_id);
    // })
}
fn filter_instances_by_tag_presence(
    instances: &[Instance],
    tag: &Tag,
    present: bool,
) -> Vec<Instance> {
    instances
        .to_vec()
        .into_iter()
        .filter(|i| {
            i.tags()
                .iter()
                .map(|t| t.key().unwrap())
                .collect::<Vec<_>>()
                .contains(&tag.key().unwrap_or_default())
                == present
        })
        .to_owned()
        .clone()
        .collect::<Vec<_>>()
}

async fn get_all_tags(client: &aws_sdk_ec2::Client) {
    let filter = Filter::builder()
        .name("resource-type".to_string())
        .values("instance".to_string())
        .build();
    let mut response = client
        .describe_tags()
        .filters(filter)
        .into_paginator()
        .page_size(5)
        .send();

    loop {
        match response.next().await {
            Some(page) => match page {
                Ok(tags) => {
                    for tag in tags.tags.unwrap_or_default() {
                        println!("tag: {:?}", tag);
                    }
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                }
            },
            None => break,
        }
    }
}
fn get_all_instance_ids(instances: &[Instance]) -> Option<Vec<String>> {
    instances
        .into_iter()
        .map(|i| i.instance_id.clone())
        .collect()
}
fn get_all_instance_names(instances: &[Instance]) -> Option<Vec<String>> {
    let mut names = vec![];
    for instance in instances {
        instance
            .tags()
            .iter()
            .filter(|t| t.key() == Some("Name"))
            .for_each(|t| names.push(t.value().unwrap().to_string()))
    }
    Some(names)
}

async fn get_all_instances(client: &aws_sdk_ec2::Client) -> Option<Vec<Instance>> {
    let mut paginator = client
        .describe_instances()
        .into_paginator()
        .page_size(5)
        .send();
    let mut instances = vec![];
    while let Some(output) = paginator.next().await {
        let Ok(output) = output else {
            eprintln!("Error while getting instance information: {:?}", output);
            continue;
        };
        let reservations = output.reservations();
        for reservation in reservations {
            let instance_slice = reservation.instances();
            instance_slice.iter().for_each(|instance| {
                instances.push(instance.clone());
            })
        }
    }
    if instances.is_empty() {
        return None;
    }
    Some(instances)
}
