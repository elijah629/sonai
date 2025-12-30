#![deny(clippy::all)]

use aho_corasick::AhoCorasick;
use linfa_clustering::KMeans;
use linfa_nn::distance::Distance;
use linfa_nn::distance::L2Dist;
use ndarray::{Array1, Array2, ArrayView1, Axis};
use pulldown_cmark::Event;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use pulldown_cmark::TagEnd;
use serde::Serialize;
use std::fmt;
use unicode_segmentation::UnicodeSegmentation;

pub type DistanceFunction = L2Dist;
pub const DIST_FN: DistanceFunction = L2Dist;

#[derive(Debug, Serialize)]
pub struct TextMetrics {
    // higher = more AI-like
    pub emoji_rate: f64,    // Emoji * 2 / sentences
    pub buzzword_rate: f64, // Buzzwords
    //
    pub not_just_count: f64,              // It's not just _, it's _
    pub html_escape_count: f64,           // &amp;
    pub devlog_count: f64,                // Devlog #whatever
    pub backstory_count: f64,             // I built this for the people of America.
    pub incorrect_perspective: f64, // We, they, you, etc
    pub human_informality: f64,              // I amss quite@ ps-rofficient in Englissh grammaeear!

    pub irregular_ellipsis: f64,   // bad ellipses
    pub irregular_quotations: f64, // Fancy quotation marks / total quotation marks
    pub irregular_dashes: f64,     // Em-dashes / total dashes
    pub irregular_markdown: f64,   // bad markdown syntax present
    pub irregular_arrows: f64,     // -> but the non hyphen-minus greater than version

    pub labels: f64,
    pub hashtags: f64,
}

impl fmt::Display for TextMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const COLUMNS: u8 = 2u8;

        let metrics = &[
            ("emoji", self.emoji_rate),
            ("not_just", self.not_just_count),
            ("buzzword", self.buzzword_rate),
            ("html", self.html_escape_count),
            ("irr_ell", self.irregular_ellipsis),
            ("irr_quote", self.irregular_quotations),
            ("irr_dash", self.irregular_dashes),
            ("irr_arr", self.irregular_arrows),
            ("irr_md", self.irregular_markdown),
            ("informal", self.human_informality),
            ("bad_per", self.incorrect_perspective),
            ("devlog", self.devlog_count),
            ("labels", self.labels),
            ("hashtags", self.hashtags),
            ("backstory", self.backstory_count),
        ];

        let mut cell = 0u8;

        for &(metric, value) in metrics {
            if value == 0. {
                continue;
            }

            let row = cell / COLUMNS;
            let col = cell % COLUMNS;

            match (row, col) {
                (0, 0) => {
                    write!(f, "{metric:<10}")?;
                }
                (_, 0) => {
                    write!(f, "          {metric:<10}")?;
                }
                (_, _) => {
                    write!(f, "\t\t{metric:<10}")?;
                }
            }

            let fractional = value.fract() != 0.;

            if fractional {
                write!(f, "{value:.1}")?;
            } else {
                write!(f, "{value}")?;
            }

            if col + 1 == COLUMNS {
                writeln!(f)?;
            }

            cell += 1;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct TextMetricFactory {
    buzzword_ahocorasick: AhoCorasick,
    negative_buzzword_ahocorasick: AhoCorasick,
    not_just_ahocorasick: AhoCorasick,
    devlog_ahocorasick: AhoCorasick,
    irr_ell_ahocorasick: AhoCorasick,
    backstory_ahocorasick: AhoCorasick,
    negative_backstory_ahocorasick: AhoCorasick,
    incorrect_perspective_ahocorasick: AhoCorasick,
    broken_english_ahocorasick: AhoCorasick,
    mr_fancy_pants_ahocorasick: AhoCorasick,
}

impl TextMetricFactory {
    pub fn new() -> Result<Self, aho_corasick::BuildError> {
        Ok(Self {
            buzzword_ahocorasick: AhoCorasick::new(include!("lists/buzzword.rs"))?,
            negative_buzzword_ahocorasick: AhoCorasick::new(include!(
                "lists/negative_buzzword.rs"
            ))?,
            broken_english_ahocorasick: AhoCorasick::new(include!("lists/broken_english.rs"))?,
            mr_fancy_pants_ahocorasick: AhoCorasick::new(["(e.g.", "(formerly", "role- "])?,
            not_just_ahocorasick: AhoCorasick::new(include!("lists/not_just.rs"))?,
            devlog_ahocorasick: AhoCorasick::new(include!("lists/devlog.rs"))?,
            irr_ell_ahocorasick: AhoCorasick::new(["…", "..."])?,
            incorrect_perspective_ahocorasick: AhoCorasick::new(include!(
                "lists/incorrect_perspective.rs"
            ))?,
            backstory_ahocorasick: AhoCorasick::new(include!("lists/backstory.rs"))?,
            negative_backstory_ahocorasick: AhoCorasick::new(include!("lists/negative_backstory.rs"))?,
        })
    }

    pub fn calculate_iter<I, S>(&self, texts: I) -> impl Iterator<Item = TextMetrics>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        texts.into_iter().map(|s| self.calculate(s.as_ref()))
    }

    pub fn calculate(&self, text: &str) -> TextMetrics {
        // existing markdown vs non-markdown

        // slow but fine, only one.
        let html_escapes = text.matches("&amp;").count();

        let mut markdown = text.matches(['•', '●']).count(); // Lists are OK, this shit is not
        let mut cleaned_text = String::new();
        let mut in_code_block = false;

        for event in Parser::new(text) {
            if matches!(
                event,
                Event::Rule
                    | Event::Start(
                        Tag::BlockQuote(_)
                            | Tag::Emphasis
                            | Tag::Subscript
                            | Tag::Superscript
                            | Tag::Strong
                            | Tag::Strikethrough
                            | Tag::Heading { .. }
                            | Tag::Link { .. }
                            | Tag::Image { .. }
                    )
            ) {
                markdown += 1;
            }

            match event {
                Event::Start(Tag::CodeBlock(_)) => in_code_block = true,
                Event::End(TagEnd::CodeBlock) => in_code_block = false,
                Event::Text(t) if !in_code_block => cleaned_text.push_str(&t),
                Event::SoftBreak | Event::HardBreak if !in_code_block => cleaned_text.push(' '),
                _ => {}
            }
        }

        let text = cleaned_text.trim().replace("\n\n", "\n");

        let mut noncap_sentences = 0;

        let sentence_count = text
            .split(['.', '!', '?', '\n'])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .inspect(|sentence| {
                if let Some(first_char) = sentence.chars().next()
                    && first_char.is_ascii() && !first_char.is_uppercase()
                {
                    noncap_sentences += 1;
                }
            })
            .count()
            .max(1);

        let text = text.to_ascii_lowercase();

        let mut labels = 0usize;

        for line in text.lines() {
            if let Some((label, after)) = line.split_once(':') {
                if !after.trim().is_empty() {
                    continue;
                }

                let label = label.trim();

                if !label.is_empty()
                    && label
                        .chars()
                        .all(|c| c.is_alphabetic() || c.is_whitespace()) && !matches!(label, "https" | "http")
                {
                    labels += 1;
                }
            }
        }

        let text = text.replace("\n", " ").replace("  ", " ");

        let words = text.split_whitespace().filter(|w| !w.trim().is_empty());

        let mut hashtags = 0usize;
        for word in words {
            if word.starts_with('#') && word.len() > 1 {
                hashtags += 1;
            }
        }

        let mut emoji_count = 0;
        let mut irr_dash = 0;
        let mut irr_quote = 0;
        let mut irr_arr = 0;

        for grapheme in text.graphemes(true) {
            if emojis::get(grapheme).is_some() && !matches!(grapheme, "😭" | "😉" | "🫣") {
                emoji_count += 1;
                continue;
            }

            let mut iter = grapheme.chars().peekable();

            while let Some(c) = iter.next() {
                match c {
                    '–' | '—' | '‒' | '―' | '⸻' | '⸺' | '−' | '﹘' | '－' | '‑' | '‐' | '᠆'
                    | '־' | '֊' => irr_dash += 1,
                    '→' | '↑' | '↓' | '↔' | '↕' | '⇒' | '⇐' | '⇑' | '⇓' | '➔' | '➜' => {
                        irr_arr += 1
                    }
                    '“' | '”' | '‘' | '’' => irr_quote += 1,
                    '-' => {
                        if iter.peek().is_some_and(|x| !x.is_whitespace()) {
                            irr_dash += 1;
                        }
                    }
                    _ => {}
                }
            }
        }

        // tradeoff is fine for a match list this small
        let irr_ell = self.irr_ell_ahocorasick.find_iter(&text).count();

        let sc = sentence_count as f64;

        let dev_log = self.devlog_ahocorasick.find_iter(&text).count();

        let buzzwords = self.buzzword_ahocorasick.find_iter(&text).count() as f64
            - self.negative_buzzword_ahocorasick.find_iter(&text).count() as f64;

        let not_just = self.not_just_ahocorasick.find_iter(&text).count();

        let backstory = self.backstory_ahocorasick.find_iter(&text).count() as f64 - self.negative_backstory_ahocorasick.find_iter(&text).count() as f64;
        let incper = self
            .incorrect_perspective_ahocorasick
            .find_iter(&text)
            .count();

        // fancy can also be interpreted as proper english. trailing commas are NOT proper english
        let informality = (if text.ends_with(",") { 1. } else { 0. })
            + self.broken_english_ahocorasick.find_iter(&text).count() as f64
            + 1.5 * noncap_sentences as f64
            - self.mr_fancy_pants_ahocorasick.find_iter(&text).count() as f64;

        TextMetrics {
            emoji_rate: (emoji_count as f64) / sc,
            buzzword_rate: buzzwords / sc,
            backstory_count: backstory,
            incorrect_perspective: (incper as f64) /sc,
            human_informality: informality / sc,

            devlog_count: dev_log as f64,
            html_escape_count: html_escapes as f64,
            not_just_count: not_just as f64,

            irregular_quotations: (irr_quote as f64) / sc,
            irregular_dashes: irr_dash as f64,
            irregular_arrows: irr_arr as f64,
            irregular_ellipsis: irr_ell as f64,
            irregular_markdown: markdown as f64,

            labels: labels as f64,
            hashtags: hashtags as f64,
        }
    }
}

pub fn features_from_metrics(data: &[&TextMetrics]) -> Array2<f64> {
    let n_features = 15;
    let n_samples = data.len();

    let mut array = Array2::<f64>::zeros((n_samples, n_features));

    for (i, sample) in data.iter().enumerate() {
        array[[i, 0]] = sample.emoji_rate;
        array[[i, 1]] = sample.buzzword_rate;
        array[[i, 2]] = sample.irregular_dashes;
        array[[i, 3]] = sample.irregular_quotations;
        array[[i, 4]] = sample.labels;
        array[[i, 5]] = sample.irregular_ellipsis;
        array[[i, 6]] = sample.html_escape_count;
        array[[i, 7]] = sample.not_just_count;
        array[[i, 8]] = sample.devlog_count;
        array[[i, 9]] = sample.irregular_markdown;
        array[[i, 10]] = sample.hashtags;
        array[[i, 11]] = sample.human_informality;
        array[[i, 12]] = sample.incorrect_perspective;
        array[[i, 13]] = sample.backstory_count;
        array[[i, 14]] = sample.irregular_arrows;
    }

    array
}

pub fn point_confidence(
    model: &KMeans<f64, DistanceFunction>,
    observation: ArrayView1<f64>,
) -> (Array1<f64>, Array1<f64>) {
    let centroids = model.centroids();
    let distances = centroids
        .axis_iter(Axis(0))
        .map(|centroid_row| DIST_FN.distance(observation, centroid_row))
        .collect::<Array1<_>>();

    let mut sims = distances.mapv(|d| 1.0 / (1.0 + d));
    let sum: f64 = sims.sum();
    if sum > 0.0 {
        sims /= sum;
    }
    (distances, sims)
}
