use std::collections::HashMap;

use bincode::config::standard;

use bincode::serde::{decode_from_slice, encode_to_vec};
use colored::Colorize;
use linfa::Dataset;
use linfa::traits::{Fit, Predict};
use linfa_clustering::KMeans;
use ndarray::{Array1, Array2};
use num_format::{Locale, ToFormattedString};
use rand::seq::IndexedRandom;
use rand_xoshiro::Xoshiro256PlusPlus;
use rand_xoshiro::rand_core::SeedableRng;
use time::{OffsetDateTime, format_description};
use tokio::fs;

mod flavortown;

use crate::flavortown::fetch_all;
use sonai_metrics::{DIST_FN, DistanceFunction, features_from_metrics};
use sonai_metrics::{TextMetricFactory, TextMetrics};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = standard();

    println!("Fetching projects + devlogs");

    let mut data: Vec<String> = if fs::try_exists("ftwn.data").await? {
        let data = fs::read("ftwn.data").await?;
        let result: Vec<String> = decode_from_slice(&data, config)?.0;

        result
    } else {
        let env_map = dotenvy::EnvLoader::new().load()?;
        let logs = fetch_all(&env_map.var("FLAVORTOWN_API_KEY")?).await?;

        fs::write("ftwn.data", encode_to_vec(&logs, config)?).await?;

        logs
    };

    let som_data: Vec<String> = if fs::try_exists("som.data").await? {
        let data = fs::read("som.data").await?;
        let result: Vec<String> = decode_from_slice(&data, config)?.0;

        result
    } else {
        vec![]
    };

    data.extend(som_data.into_iter());

    println!("Calculating metrics");
    let metrics: Vec<TextMetrics> = TextMetricFactory::new()?.calculate_iter(&data).collect();
    let metrics_refs: Vec<&TextMetrics> = metrics.iter().collect();
    let features = features_from_metrics(&metrics_refs);

    println!("Building dataset");
    let dataset = Dataset::new(features.clone(), Array2::<f32>::zeros((metrics.len(), 0)));

    let rng = Xoshiro256PlusPlus::seed_from_u64(0x7F3A_9C1D_4B2E_6F80);

    println!("Training");
    let model: KMeans<f64, DistanceFunction> = KMeans::params_with(2, rng, DIST_FN)
        .max_n_iterations(1000)
        .n_runs(10)
        .fit(&dataset)?;

    fs::write("../sonai/model.kmeans", encode_to_vec(&model, config)?).await?;

    println!("Predicting");
    let predicted: Array1<usize> = model.predict(&features);

    let (emoji_sums, counts) = metrics.iter().zip(predicted.iter()).fold(
        ([0.0f64; 2], [0usize; 2]),
        |(mut current_emoji_sums, mut current_counts), (metric, &label)| {
            current_emoji_sums[label] += metric.emoji_rate;
            current_counts[label] += 1;
            (current_emoji_sums, current_counts)
        },
    );

    let avg_emoji = [
        emoji_sums[0] / (counts[0].max(1) as f64),
        emoji_sums[1] / (counts[1].max(1) as f64),
    ];

    let ai_label = if avg_emoji[0] > avg_emoji[1] { 0 } else { 1 };
    let human_label = if avg_emoji[0] > avg_emoji[1] { 1 } else { 0 };

    fs::write("../sonai/model.ai.cluster", [ai_label as u8]).await?;

    let cluster_counts: [usize; 2] = predicted.iter().fold([0, 0], |mut counts, &label| {
        counts[label] += 1;
        counts
    });

    let ai = cluster_counts[ai_label];
    let human = cluster_counts[human_label];
    let total = ai + human;

    let mut clusters: HashMap<usize, Vec<(TextMetrics, String)>> = HashMap::new();

    for ((label, metrics), devlog) in predicted.into_iter().zip(metrics).zip(data) {
        clusters.entry(label).or_default().push((metrics, devlog));
    }

    let mut rng = rand::rng();

    for (label, items) in clusters {
        println!(
            "\n{}",
            format!("==================== Cluster {label} ====================")
                .bold()
                .cyan()
        );

        let sample = items.choose_multiple(&mut rng, 5);

        for (i, (metrics, devlog)) in sample.into_iter().enumerate() {
            println!("{}", format!("--- Sample {i} ---").bold().yellow());
            println!("{} {}", "Features:".green(), metrics);
            println!("{}\n{}", "Text:".blue(), devlog);
            println!("{}", "-------------------------------\n".dimmed());
        }
    }

    let human_pct = (human as f64) * 100. / (total as f64);
    let ai_pct = (ai as f64) * 100. / (total as f64);

    println!("ai_cluster={ai_label} human=({human_pct:.2}%, {human}) ai=({ai_pct:.2}%, {ai})",);

    let human = human.to_formatted_string(&Locale::en);
    let ai = ai.to_formatted_string(&Locale::en);

    let date = OffsetDateTime::now_utc()
        .format(
            &format_description::parse("[month repr:short] [day padding:none], [year]")
                .expect("valid format description"),
        )
        .expect("today is a day");

    let file = format!(
        r##"<!-- Do not change this file manually, please edit the template string at the bottom of training-bin/src/main.rs and rebuild  -->
<!doctype html>
<html lang="en" class="dark">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>How much of Flavortown is AI?</title>
  </head>
  <body
    class="bg-gray-100 text-gray-800 dark:bg-gray-900 dark:text-gray-100 antialiased">
    <header class="bg-white dark:bg-gray-800 shadow-md">
      <div class="max-w-5xl mx-auto py-6 px-5 flex items-center justify-between">
        <h1 class="text-3xl font-semibold">sonai Detector Demo</h1>
        <a
          href="https://github.com/elijah629/sonai"
          target="_blank"
          class="text-blue-600 underline hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300"
          >Source</a
        >
      </div>
    </header>
    <main class="max-w-5xl mx-auto p-6 space-y-8">
      <section class="bg-white dark:bg-gray-800 rounded-lg shadow-sm p-6">
        <h2 class="text-2xl font-medium mb-4">
          Projects + Devlog stats as of
          <span class="font-semibold">{date}</span>:
        </h2>
        <div class="flex flex-wrap gap-4 text-lg">
          <div class="flex items-center space-x-2">
            <span class="font-semibold">Human:</span>
            <span class="text-green-600 dark:text-green-400">{human}</span>
          </div>
          <div class="flex items-center space-x-2">
            <span class="font-semibold">AI:</span>
            <span class="text-blue-600 dark:text-blue-400">{ai}</span>
          </div>
          <div class="flex items-center space-x-2">
            <span class="font-semibold">Human %:</span>
            <span class="text-green-600 dark:text-green-400">{human_pct:.2}%</span>
          </div>
          <div class="flex items-center space-x-2">
            <span class="font-semibold">AI %:</span>
            <span class="text-blue-600 dark:text-blue-400">{ai_pct:.2}%</span>
          </div>
        </div>
      </section>

      <section class="bg-white dark:bg-gray-800 rounded-lg shadow-sm p-6">
        <h2 class="text-2xl font-medium mb-4">Check Your Own Devlogs!</h2>
        <div class="flex flex-col md:flex-row gap-4">
          <textarea
            id="input"
            class="flex-1 resize-y grow min-h-[500px] p-4 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-900 focus:outline-none focus:ring-2 focus:ring-blue-500 dark:focus:ring-blue-400 scrollbar-hide"
            placeholder="Type here, AI checks live (The model is very fast and runs entirely in your browser with WASM!)"
          ></textarea>
          <pre
            id="output"
            class="w-min p-4 border border-gray-300 dark:border-gray-600 rounded-lg bg-gray-50 dark:bg-gray-900 min-w-sm"
          ></pre>
        </div>
      </section>
    </main>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>"##
    );

    fs::write("../inference-wasm-web/index.html", file).await?;

    Ok(())
}
