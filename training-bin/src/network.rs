use anyhow::{Result, anyhow};
use reqwest::header::RETRY_AFTER;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tokio::task::JoinSet;
use tokio::time::sleep;

#[derive(Deserialize, Clone)]
pub struct Pagination {
    current_page: u64,
    total_pages: u64,
    total_count: u64,
    next_page: u64,
}

pub trait Pagintated: for<'a> Deserialize<'a> {
    const ROOT: &'static str;
    type Data: for<'a> Deserialize<'a> + Clone + Send + Sync + 'static;

    fn page(self) -> Vec<Self::Data>;
    fn pagination(&self) -> &Pagination;
}

pub async fn concurrent_pagintated_retry_fetch<P: Pagintated>(
    client: &Client,
) -> Result<Vec<P::Data>> {
    let root = P::ROOT;

    println!("[fetch] starting paginated fetch for {}", root);

    let first = fetch_single_wrapped::<5, P, _>(client, format!("{root}?page=1")).await?;

    let Pagination {
        total_pages,
        total_count,
        ..
    } = first.pagination();

    println!(
        "[fetch] first page fetched: total_pages={}, total_count={}",
        total_pages, total_count
    );

    let total_pages = *total_pages as usize;

    let mut results = vec![None; total_pages];
    results[0] = Some(first.page());

    let results = Arc::new(RwLock::new(results));

    let mut pending_pages: Vec<usize> = (2..=total_pages).collect();

    let (retry_tx, mut retry_rx) = mpsc::channel::<Duration>(1);

    while !pending_pages.is_empty() {
        println!(
            "[round] starting round with {} pending pages: {:?}",
            pending_pages.len(),
            pending_pages,
        );

        let mut join_set = JoinSet::new();

        for &page in &pending_pages {
            let url = format!("{root}?page={page}");
            let client = client.clone();
            let retry_tx = retry_tx.clone();
            let results = results.clone();

            println!("[spawn] spawning task for page {}", page);
            join_set.spawn(async move {
                let result = fetch_single::<P>(client, &url, retry_tx).await;
                let mut results_guard = results.write().await;
                results_guard[page - 1] = result;
            });
        }

        // Race: either all complete or a retry is triggered
        loop {
            tokio::select! {
                Some(result) = join_set.join_next() => {
                    if result.is_err() {
                        // Task panicked or was cancelled
                        continue;
                    }

                    if join_set.is_empty() {
                        println!("[round] all tasks completed successfully this round");
                        pending_pages.clear();
                        break;
                    }
                }
                Some(duration) = retry_rx.recv() => {
                    println!("[retry] received retry signal: waiting {:?} before retrying", duration);

                    join_set.shutdown().await;
                    sleep(duration).await;

                    // Update pending list - only retry URLs that didn't complete
                    let results_guard = results.read().await;
                    pending_pages = results_guard.iter()
                        .enumerate()
                        .filter_map(|(i, r)| if r.is_none() { Some(i + 1) } else { None })
                        .collect();

                    break;
                }
            }
        }
    }

    let final_results = results.read().await;
    let final_results: Vec<P::Data> = final_results.iter().flatten().flatten().cloned().collect();

    println!(
        "[fetch] completed paginated fetch: total_items={} (pages={})",
        final_results.len(),
        total_pages,
    );

    Ok(final_results)
}

/// Fetches a single URL with retry-after detection.
///
/// Sends a retry signal via channel if a 429 with Retry-After is encountered.
/// The channel's capacity of 1 ensures only the first retry signal is processed.
async fn fetch_single<P: Pagintated>(
    client: reqwest::Client,
    url: &str,
    retry_tx: mpsc::Sender<Duration>,
) -> Option<Vec<P::Data>> {
    let response = client.get(url).send().await.ok()?;
    let status = response.status();

    println!("[request] GET {} -> {}", url, status);

    match status {
        StatusCode::TOO_MANY_REQUESTS => {
            println!("[429] rate limited for {}", url);
            if let Some(duration) = parse_retry_after(response.headers().get(RETRY_AFTER)) {
                println!("[429] retry-after = {:?} for {}", duration, url);
                let _ = retry_tx.try_send(duration);
            }
            None
        }
        status if status.is_success() => {
            println!("[success] parsed JSON for {}", url);
            response.json::<P>().await.ok().map(|x| x.page())
        }
        _ => {
            println!("[error] unexpected status {} for {}", status, url);
            None
        }
    }
}

fn parse_retry_after(header: Option<&reqwest::header::HeaderValue>) -> Option<Duration> {
    let header = header?;
    let s = header.to_str().ok()?;

    if let Ok(seconds) = s.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }

    // api returns in seconds but just in case
    if let Ok(http_date) = httpdate::parse_http_date(s) {
        let now = std::time::SystemTime::now();
        if let Ok(duration) = http_date.duration_since(now) {
            return Some(duration);
        }
    }

    None
}

async fn fetch_single_wrapped<const RETRIES: usize, P: Pagintated, T: AsRef<str>>(
    client: &reqwest::Client,
    url: T,
) -> Result<P> {
    let url = url.as_ref();
    let mut attempt = 0;

    loop {
        let response = client.get(url).send().await;

        match response {
            Ok(resp) => {
                if resp.status() == StatusCode::TOO_MANY_REQUESTS
                    && let Some(delay_secs) = parse_retry_after(resp.headers().get(RETRY_AFTER))
                {
                    println!(
                        "[rate-limit] attempt {}/{} — waiting {:?} before retry",
                        attempt + 1,
                        RETRIES,
                        delay_secs
                    );

                    sleep(delay_secs).await;
                    continue;
                }

                match resp.json::<P>().await {
                    Ok(json) => {
                        println!("[success] fetched {} after {} attempt(s)", url, attempt + 1);
                        return Ok(json);
                    }
                    Err(err) => {
                        attempt += 1;
                        println!(
                            "[error] attempt {}/{} — JSON parse failed: {}",
                            attempt, RETRIES, err
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
                    attempt, RETRIES, err
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

        let backoff = Duration::from_millis(500 * 2_u64.pow(attempt.saturating_sub(1) as u32));
        println!(
            "[retry] backing off for {}ms before next attempt",
            backoff.as_millis()
        );

        sleep(backoff).await;
    }
}
