# how much of som is ai?

I trained a 271 + 249 + 1 (521) byte 15 parameter AI Detection KMeans model on
every SoM devlog. Here's how it went!

## How this version works

### Functional Training

- Get every devlog from SoM api
- Calculate text features
- KMeans training
- Export model
- Meause the model against its own training data to see how much of SoM is AI.

### Functional Inference

- Load the previously exported model
- Compute text features
- Perform a prediction with the features and exported model

## My whole brainsstorming era

First I made an embedder using L12 MiniLM on huggingface. This suprisingly
worked first try, embedding every single devlog in under 30s (credit to my super
computer). Due to the popular "Everything is AI whyyy" complaints from hackclub
members, I then attempted to measure how much of the devlogs were written by AI.
I decided to use the `linfa` ecosystem by the `rust-ml` group. I use a KMeans
algorithim to classify the text into two unlabeled clusters based off of a
feature matrix. I calculate these features:

```rust
struct TextMetrics {
    // higher = more AI-like
    pub emoji_rate: f64, // Emoji * 2 / sentences
    pub buzzword_rate: f64,    // Buzzwords

    pub not_just_count: f64,    // It's not just _, it's _
    pub html_escape_count: f64, // &amp;
    pub devlog_count: f64,      // Devlog #whatever

    pub irregular_ellipsis: f64,   // bad ellipses
    pub irregular_quotations: f64, // Fancy quotation marks / total quotation marks
    pub irregular_dashes: f64,     // Em-dashes / total dashes
    pub irregular_markdown: f64,   // bad markdown syntax present

    pub labels: f64,
    pub hashtags: f64,
}
```

I then piped them all into a KMeans model and trained it a few (10 billion)
times.

## Library

You can run the latest pre-trained version of the model in your own projects
like this

```sh
cargo add sonai # Summer of No AI
```

```rust
// This code works in WASM too!

use sonai::{predict, Prediction};

fn main() {
    let Prediction { chance_ai, chance_human } = predict("Hello, world!");

    let chance_ai = chance_ai * 100;
    let chance_human = chance_human * 100;

    println!("{chance_ai}% ai, {chance_human}% human");
}
```

## DIY

### Project-structure

- `training-bin` Training, generates a model.kmeans and model.ai.cluster inside
  the `sonai` crate.
- `sonai"` Runs a model.kmeans and performs predictions. This can be installed
  as a library in wasm and non-wasm environments.
- `sonai-metrics` Helper lib to calculate text metrics

Place `JOURNEY=` in `training-bin/.env` to fetch devlogs & projects, or use the
provided `training-bin/som.data` file.

### Training

To train the model, run the binary in `training-bin`

```sh
cd training-bin
cargo r -r
```

### WASM

For demo purposes, this crate has been ported to WASM and a static site where
you can run the AI detection model on your own text. Compile the wasm demo
yourself with:

> [!NOTE]
> wasm-pack & rustwasm has been deprecated, the old way is still here for legacy
> reasons
>
> ```sh
> cd sonai
> wasm-pack build --release -d ../inference-wasm-web/src/pkg
> ```
>
> All the opt flags have been preconfigured in `Cargo.toml`

```sh
cd sonai
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen ../target/wasm32-unknown-unknown/release/sonai.wasm --out-dir ../inference-wasm-web/src/pkg --target bundler
wasm-opt -O4 --strip-debug --enable-bulk-memory-opt -o ../inference-wasm-web/src/pkg/sonai_bg.wasm ../inference-wasm-web/src/pkg/sonai_bg.wasm
```

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
</sub>
