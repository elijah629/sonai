use std::{fmt::Display, time::Duration};

use anyhow::{Result, anyhow};
use futures::{StreamExt, TryStreamExt};
use reqwest::{IntoUrl, StatusCode, header::{AUTHORIZATION, COOKIE, HeaderMap, HeaderName, HeaderValue, RETRY_AFTER}};
use serde::Deserialize;
use tokio::time::sleep;

#[derive(Deserialize)]
pub struct Devlogs {
    devlogs: Vec<Devlog>,
    pagination: Pagination,
}

#[derive(Deserialize)]
pub struct Devlog {
      body: String,
}


#[derive(Deserialize)]
pub struct Projects {
    projects: Vec<Project>,
    pagination: Pagination,
}

#[derive(Deserialize)]
pub struct Project {
      id: u64,
      description: String,
      devlog_ids: Vec<u64>,
}

#[derive(Deserialize)]
pub struct Pagination {
    // current_page: u64,
    total_pages: u64,
    total_count: u64,
    // next_page: u64,
}

async fn fetch_json<const RETRIES: usize, T, U: AsRef<str>>(
    client: &reqwest::Client,
    url: U,
) -> Result<T>
where
    T: for<'a>  Deserialize<'a>,
{
    let url = url.as_ref();

    let mut attempt = 0;

    loop {
        let response = client.get(url).send().await;

        match response {
            Ok(resp) => {
                if resp.status() == StatusCode::TOO_MANY_REQUESTS && 
                    let Some(delay_secs) = resp
                        .headers()
                        .get(RETRY_AFTER)
                        .and_then(|h| h.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                {
                    println!(
                        "[rate-limit] attempt {}/{} — waiting {}s before retry",
                        attempt + 1,
                        RETRIES,
                        delay_secs
                    );

                    sleep(Duration::from_secs(delay_secs)).await;
                    continue;
                }



                match resp.json::<T>().await {
                    Ok(json) => {
                        println!(
                            "[success] fetched {} after {} attempt(s)",
                            url,
                            attempt + 1
                        );
                        return Ok(json);
                    }
                    Err(err) => {
                        attempt += 1;
                        println!(
                            "[error] attempt {}/{} — JSON parse failed: {}",
                            attempt,
                            RETRIES,
                            err
                        );

                        if attempt >= RETRIES {
                            return Err(anyhow!(
                                "JSON parse error after {} attempts: {}",
                                RETRIES,
                                err
                            ));
                        }
                    }
                }
            }
            Err(err) => {
                attempt += 1;
                println!(
                    "[error] attempt {}/{} — request failed: {}",
                    attempt,
                    RETRIES,
                    err
                );

                if attempt >= RETRIES {
                    return Err(anyhow!(
                        "Request failed after {} attempts: {}",
                        RETRIES,
                        err
                    ));
                }
            }
        }

        let backoff = Duration::from_millis(500 * 2_u64.pow(attempt as u32 - 1));
        println!(
            "[retry] backing off for {}ms before next attempt",
            backoff.as_millis()
        );

        sleep(backoff).await;
    }
}


pub async fn fetch_all(api_key: &str) -> Result<Vec<String>> {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(format!("Bearer {api_key}").as_str())?,
    );

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    let first = fetch_json::<5, Projects, _>(&client, "https://flavortown.hackclub.com/api/v1/projects?page=1").await?;

    let Pagination { total_count, total_pages, .. } = first.pagination;

    let mut projects= Vec::with_capacity(total_count as usize);
    projects.extend(first.projects);

    println!("{total_pages} to fetch. ETA {}m", (total_pages as f32 / 60.));

    for page in 2..=total_pages {
        let url = format!("https://flavortown.hackclub.com/api/v1/projects?page={page}");

        let page = fetch_json::<5, Projects, _>(&client, &url).await?;

        projects.extend(page.projects);
    }
    
    let mut text_data = Vec::with_capacity(total_pages as usize);

    let total_devlogs: usize = projects.iter().map(|x| x.devlog_ids.len()).sum();

    println!("{total_devlogs} to fetch. ETA {}m", (total_devlogs as f32 / 60.));

    for project in projects {
        let real_desc = project.description.replace("This is my first project on Flavortown. I'm excited to share my progress!", "");
        let real_desc = real_desc.trim();

        if !real_desc.is_empty() {
            text_data.push(real_desc.to_string());
        }

        /*for devlog_id in project.devlog_ids {
            let url = format!("https://flavortown.hackclub.com/api/v1/projects/{}/devlogs/{}", project.id, devlog_id);
            let devlog = fetch_json::<5, Devlog, _>(&client, url).await?;

            text_data.push(devlog.body);
        }*/
    }

    Ok(text_data)
}
