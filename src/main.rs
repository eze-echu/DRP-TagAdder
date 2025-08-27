#![no_std]
extern crate alloc;

mod cli;
use cli::cli;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use aws_config::BehaviorVersion;
use aws_config::stalled_stream_protection::StalledStreamProtectionConfig;
use aws_sdk_ec2::types::{Filter, Tag};
#[cfg(feature = "tracing")]
use log::{error, info, trace};
use tokio::time;

#[tokio::main]
async fn main() {
    let matches = cli().get_matches();
    let profile = matches.get_one::<String>("profile").unwrap();
    let drp_tier = matches.get_one::<String>("drp-tier").unwrap();
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
    add_tags_to_all_instances(&client, drp_tier).await;
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
        Ok(a) => {
            #[cfg(feature = "tracing")]
            info!("{:?}", a);
        }
        Err(e) => {
            #[cfg(feature = "tracing")]
            error!("Error: {:?}", e);
        }
    }
}
async fn add_tags_to_all_instances(client: &aws_sdk_ec2::Client, drp_tier: &str) {
    let instance_id_vec = match get_all_instances(client).await {
        Some(a) => a,
        None => {
            //    eprintln!("No instances found");
            return;
        }
    };
    let tag = Tag::builder().key("DRPBackupPlan").value(drp_tier).build();
    for instance_id in instance_id_vec {
        add_tag_to_instance(client, &tag, &instance_id).await;
    }
    // instance_id_vec.par_iter().for_each(|instance_id| {
    //     let _ = add_tag_to_instance(client, &tag, &instance_id);
    // })
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
                        //println!("tag: {:?}", tag);
                    }
                }
                Err(err) => {
                    //println!("Error: {:?}", err);
                }
            },
            None => break,
        }
    }
}
async fn get_all_instances(client: &aws_sdk_ec2::Client) -> Option<Vec<String>> {
    let mut instance_id_vec = vec![];
    let mut request = client
        .describe_instances()
        .into_paginator()
        .page_size(5)
        .send();

    while let Some(output) = request.next().await {
        let output = match output {
            Ok(output) => output,
            Err(e) => {
                //eprintln!("{:?}", e);
                continue;
            }
        };

        let reservations = output.reservations?;

        for reservation in reservations {
            for instance in reservation.instances? {
                let id = instance.instance_id()?.to_string();
                (&mut instance_id_vec).push(instance.instance_id?);
            }
        }
    }
    if instance_id_vec.is_empty() {
        return None;
    }
    Some(instance_id_vec)
}
