extern crate alloc;
extern crate core;

mod cli;
use cli::cli;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use aws_config::BehaviorVersion;
use aws_config::stalled_stream_protection::StalledStreamProtectionConfig;
use aws_sdk_ec2::error::SdkError;
use aws_sdk_ec2::operation::RequestId;
use aws_sdk_ec2::types::{Instance, Tag};
#[cfg(feature = "tracing")]
use log::{error, info};
use tokio::time;
use anyhow::Result;
use thiserror::__private::AsDisplay;
use thiserror::Error;

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
        Some((subcommand, arg)) => match subcommand.to_lowercase().as_str() {
            "all" => {
                arg.get_one::<String>("key").unwrap();
                arg.get_one::<String>("value").unwrap();
            }
            "instance" => {
                todo!()
            }
            "drp" => {
                let drp_tier = arg.get_one::<String>("drp-tier").unwrap();

                match add_tags_to_all_instances(&client, drp_tier).await {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("{}", e);
                    }
                }
            }
            _ => {
                eprintln!("Not a valid command")
            }
        },
        None => {
            eprintln!("No subcommand provided");
        }
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
async fn add_tags_to_all_instances(
    client: &aws_sdk_ec2::Client,
    drp_tier: &str,
) -> Result<()> {
    let instances = match get_all_instances(client).await {
        Ok(instances_vec) => instances_vec,
        Err(e) => {
            eprintln!("Error: No Instances to edit");
            return Err(e.into());
        }
    };
    if instances.is_empty() {
        return Err(NoInstancesError.into());
    }
    let tag = Tag::builder().key("DRPBackupPlan").value(drp_tier).build();
    let instances = filter_instances_by_tag_presence(&instances, &tag, false);

    let mut names = get_all_instance_names(&instances).unwrap();
    names.sort();
    println!("About to edit {} instances: {:#?}", instances.len(), names);

    for instance in instances {
        let Some(instance_id) = instance.instance_id() else {
            continue;
        };
        println!("Adding Tag to instance \"{}\" ({})", get_instance_name(&instance), instance_id);
        println!("Proceed? [y/N]");
        let mut user_response = String::new();
        std::io::stdin().read_line(&mut user_response).unwrap();
        if !user_response.starts_with("y") && !user_response.starts_with("Y") {
            println!("Skipping {}", instance_id);
            continue;
        }
        add_tag_to_instance(client, &tag, &instance_id).await;
    }
    Ok(())
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
        .into_iter()
        .filter(|i| {
            i.tags()
                .iter()
                .map(|t| t.key().unwrap())
                .collect::<Vec<_>>()
                .contains(&tag.key().unwrap_or_default())
                == present
        })
        .map(|i| i.to_owned())
        .collect::<Vec<_>>()
}
fn _get_all_instance_ids(instances: &[Instance]) -> Option<Vec<String>> {
    instances
        .into_iter()
        .map(|i| i.instance_id.clone())
        .collect()
}
fn get_all_instance_names(instances: &[Instance]) -> Option<Vec<String>> {
    let mut names = vec![];
    instances
        .iter()
        .map(|i| i.tags())
        .flatten()
        .filter(|&t| t.key().unwrap() == "Name")
        .map(|t| t.value().unwrap().to_string())
        .for_each(|t| names.push(t));
    Some(names)
}
fn get_instance_name(instance: &Instance) -> String {
    instance
        .tags()
        .iter()
        .filter(|t| t.key().unwrap() == "Name")
        .map(|t| t.value().unwrap().to_string())
        .collect::<Vec<_>>()
        .first()
        .unwrap()
        .clone()
}

async fn get_all_instances(
    client: &aws_sdk_ec2::Client,
) -> Result<Vec<Instance>> {
    let paginator = client
        .describe_instances()
        .into_paginator()
        .page_size(5)
        .send();
    let mut instances: Vec<Instance> = vec![];
    match paginator.try_collect().await {
        Ok(instances_output) => {
            instances_output
                .iter()
                .map(|dio| dio.reservations())
                .flatten()
                .map(|reservations| reservations.instances())
                .flatten()
                .for_each(|i| instances.push(i.clone()));
        }
        Err(e) => {
            eprintln!("Error getting instances: {}", e);
            match &e {
                SdkError::ConstructionFailure(failure) => {
                    eprintln!("Sdk Construction Failure: {:#?}", failure);
                }
                SdkError::TimeoutError(failure) => {
                    eprintln!("Sdk Timeout Error: {:#?}", failure);
                }
                SdkError::DispatchFailure(failure) => {
                    eprintln!(
                        "Sdk Dispatch Failure (Probably expired token): {}",
                        failure.as_connector_error().unwrap().as_display()
                    );
                }
                SdkError::ResponseError(failure) => {
                    eprintln!("Sdk Response Error: {:#?}", failure.raw());
                }
                SdkError::ServiceError(failure) => {
                    eprintln!("Sdk Service Error: {:#?}", failure);
                }
                _ => {
                    eprintln!("Sdk Error: {:#?}", e);
                }
            }
            return Err(e.into());
        }
    }
    Ok(instances)
}

#[derive(Error, Debug)]
#[error("No Instances were found meeting the criteria")]
struct NoInstancesError;
