use anyhow::Result;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};

use crate::{
    flavortown::sources::{Devlogs, Projects},
    network::concurrent_pagintated_retry_fetch,
};

mod sources;

pub async fn fetch_all(api_key: &str) -> Result<Vec<String>> {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(format!("Bearer {api_key}").as_str())?,
    );

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    let projects = concurrent_pagintated_retry_fetch::<Projects>(&client).await?;

    let devlogs = concurrent_pagintated_retry_fetch::<Devlogs>(&client).await?;

    Ok(projects.into_iter().filter_map(|project| {
        let desc = project.description.replace("This is my first project on Flavortown.", "").replace("Im excited to share my progress!", "").replace("I'm excited to share my progress!", "");
        let desc = desc.trim();

        if !desc.is_empty() {
            Some(desc.to_string())
        } else {
            None
        }
    }).chain(devlogs.into_iter().filter_map(|devlog| {
            let body = devlog.body.replace("I'm working on my first project! This is so exciting. I can't wait to share more updates as I build.", "");
            let body = body.trim();

            if !body.is_empty() {
                Some(body.to_string())
            } else {
                None
            }
        })).collect())
}
