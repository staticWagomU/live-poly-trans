//! Step 0 spike: measure the ggml path (whisper-rs + llama-cpp-2) against the
//! latency budgets in plan.md, and verify both libraries co-link in one binary.
//!
//! Usage: spike [asr|translate|concurrent|all]
//! Model/audio paths are relative to the repo root (see plan.md Step 0).

use std::num::NonZeroU32;
use std::time::Instant;

use anyhow::{Context, Result};

const WHISPER_MODEL: &str = "models/ggml-large-v3-turbo-q5_0.bin";
const LLM_MODEL: &str = "models/Qwen3-4B-Instruct-2507-Q4_K_M.gguf";
const AUDIO_JA: &str = "assets/test-ja.wav";
const AUDIO_EN: &str = "assets/test-en.wav";

/// Rolling re-decode window used by the pseudo-streaming simulation.
/// Matches the LocalAgreement design: only the last N seconds are re-decoded.
const WINDOW_SECS: usize = 15;
const STEP_SECS: usize = 1;
const SAMPLE_RATE: usize = 16_000;

fn main() -> Result<()> {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "all".into());
    match mode.as_str() {
        #[cfg(feature = "asr")]
        "asr" => run_asr()?,
        #[cfg(feature = "llm")]
        "translate" => run_translate()?,
        #[cfg(all(feature = "asr", feature = "llm"))]
        "concurrent" => run_concurrent()?,
        #[cfg(all(feature = "asr", feature = "llm"))]
        "all" => {
            run_asr()?;
            run_translate()?;
            run_concurrent()?;
        }
        other => anyhow::bail!("unknown or unavailable mode: {other}"),
    }
    println!("\npeak RSS: {} MB", peak_rss_mb());
    Ok(())
}

fn peak_rss_mb() -> u64 {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    // ru_maxrss is bytes on macOS, kilobytes on Linux.
    #[cfg(target_os = "macos")]
    return (usage.ru_maxrss as u64) / (1024 * 1024);
    #[cfg(not(target_os = "macos"))]
    return (usage.ru_maxrss as u64) / 1024;
}

fn read_wav_16k_mono(path: &str) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path).with_context(|| format!("open {path}"))?;
    let spec = reader.spec();
    anyhow::ensure!(
        spec.sample_rate == 16_000 && spec.channels == 1,
        "expected 16kHz mono, got {}Hz {}ch",
        spec.sample_rate,
        spec.channels
    );
    Ok(reader
        .samples::<i16>()
        .map(|s| s.map(|v| v as f32 / 32768.0))
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

// ---------------------------------------------------------------- ASR (whisper)

#[cfg(feature = "asr")]
struct Whisper {
    ctx: whisper_rs::WhisperContext,
}

#[cfg(feature = "asr")]
impl Whisper {
    fn load() -> Result<Self> {
        let t = Instant::now();
        let model = std::env::var("WHISPER_MODEL").unwrap_or_else(|_| WHISPER_MODEL.into());
        println!("whisper model: {model}");
        let mut params = whisper_rs::WhisperContextParameters::default();
        params.use_gpu(std::env::var("WHISPER_CPU").is_err());
        let ctx = whisper_rs::WhisperContext::new_with_params(&model, params)
            .context("load whisper model")?;
        println!("whisper model loaded in {} ms", t.elapsed().as_millis());
        Ok(Self { ctx })
    }

    /// Decode one window; returns (text, wall_ms).
    fn decode(&self, samples: &[f32], lang: &str) -> Result<(String, u128)> {
        let mut state = self.ctx.create_state()?;
        let mut params =
            whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(lang));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        let t = Instant::now();
        state.full(params, samples)?;
        let wall = t.elapsed().as_millis();
        let mut text = String::new();
        for i in 0..state.full_n_segments() {
            if let Some(seg) = state.get_segment(i) {
                text.push_str(&seg.to_str_lossy()?);
            }
        }
        Ok((text.trim().to_string(), wall))
    }
}

#[cfg(feature = "asr")]
fn run_asr() -> Result<()> {
    println!("=== ASR (whisper, Metal) ===");
    let whisper = Whisper::load()?;

    for (path, lang) in [(AUDIO_JA, "ja"), (AUDIO_EN, "en"), (AUDIO_EN, "auto")] {
        let audio = read_wav_16k_mono(path)?;
        let dur_s = audio.len() as f64 / SAMPLE_RATE as f64;
        println!("\n--- {path} lang={lang} ({dur_s:.1}s) ---");

        // Full-file decode: accuracy reference + RTF.
        let (text, wall) = whisper.decode(&audio, lang)?;
        println!("full decode: {wall} ms (RTF {:.2})", wall as f64 / 1000.0 / dur_s);
        println!("text: {text}");

        // Pseudo-streaming: every STEP_SECS, re-decode the last WINDOW_SECS.
        // decode wall time is the latency a partial result would add on top
        // of the step interval.
        println!("windowed re-decode (window ≤{WINDOW_SECS}s, step {STEP_SECS}s):");
        let mut worst = 0u128;
        let mut first_text_at: Option<(usize, u128)> = None;
        for end_s in (STEP_SECS..=dur_s.ceil() as usize).step_by(STEP_SECS) {
            let end = (end_s * SAMPLE_RATE).min(audio.len());
            let start = end.saturating_sub(WINDOW_SECS * SAMPLE_RATE);
            let (text, wall) = whisper.decode(&audio[start..end], lang)?;
            worst = worst.max(wall);
            if first_text_at.is_none() && !text.is_empty() {
                first_text_at = Some((end_s, wall));
            }
            println!("  t={end_s:>2}s window={:>5.1}s decode={wall:>5} ms", (end - start) as f64 / SAMPLE_RATE as f64);
        }
        if let Some((t, wall)) = first_text_at {
            println!("first non-empty partial: audio_t={t}s + decode {wall} ms");
        }
        println!("worst window decode: {worst} ms (budget: first partial ≤2000 ms)");
    }
    Ok(())
}

// ------------------------------------------------------------ translation (llama)

#[cfg(feature = "llm")]
struct Llm {
    backend: llama_cpp_2::llama_backend::LlamaBackend,
    model: llama_cpp_2::model::LlamaModel,
}

#[cfg(feature = "llm")]
impl Llm {
    fn load() -> Result<Self> {
        let t = Instant::now();
        let backend = llama_cpp_2::llama_backend::LlamaBackend::init()?;
        let params = llama_cpp_2::model::params::LlamaModelParams::default()
            .with_n_gpu_layers(1_000_000);
        let model = llama_cpp_2::model::LlamaModel::load_from_file(&backend, LLM_MODEL, &params)
            .context("load llm model")?;
        println!("llm model loaded in {} ms", t.elapsed().as_millis());
        Ok(Self { backend, model })
    }

    /// Translate one sentence; returns (translation, wall_ms, gen_tokens).
    fn translate(&self, sentence: &str, source: &str, target: &str) -> Result<(String, u128, u32)> {
        use llama_cpp_2::model::AddBos;

        let prompt = format!(
            "<|im_start|>system\nYou are a professional simultaneous interpreter. \
             Translate the user's sentence from {source} to {target}. \
             Output only the translation, nothing else.<|im_end|>\n\
             <|im_start|>user\n{sentence}<|im_end|>\n<|im_start|>assistant\n"
        );

        let t = Instant::now();
        let ctx_params = llama_cpp_2::context::params::LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(2048));
        let mut ctx = self.model.new_context(&self.backend, ctx_params)?;

        let tokens = self.model.str_to_token(&prompt, AddBos::Never)?;
        let mut batch = llama_cpp_2::llama_batch::LlamaBatch::new(2048, 1);
        let last = tokens.len() - 1;
        for (i, token) in tokens.iter().enumerate() {
            batch.add(*token, i as i32, &[0], i == last)?;
        }
        ctx.decode(&mut batch)?;

        let mut sampler = llama_cpp_2::sampling::LlamaSampler::greedy();
        // Accumulate raw bytes: a UTF-8 sequence can be split across tokens,
        // so per-token string conversion corrupts multi-byte characters.
        let mut out_bytes: Vec<u8> = Vec::new();
        let mut pos = tokens.len() as i32;
        let mut gen = 0u32;
        loop {
            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            sampler.accept(token);
            if self.model.is_eog_token(token) || gen >= 256 {
                break;
            }
            out_bytes.extend(self.model.token_to_piece_bytes(token, 64, false, None)?);
            gen += 1;
            batch.clear();
            batch.add(token, pos, &[0], true)?;
            ctx.decode(&mut batch)?;
            pos += 1;
        }
        let out = String::from_utf8_lossy(&out_bytes);
        Ok((out.trim().to_string(), t.elapsed().as_millis(), gen))
    }
}

const EN_SENTENCES: [&str; 3] = [
    "Thank you all for joining today.",
    "The implementation of the new search feature was completed on schedule.",
    "Next week we plan to start working on performance improvements, but we may need to revisit the priorities depending on the customer feedback we receive.",
];
const JA_SENTENCES: [&str; 2] = [
    "先週の進捗を共有します。",
    "新しい検索機能の実装は予定どおり完了しましたが、パフォーマンスにはまだ改善の余地があります。",
];

#[cfg(feature = "llm")]
fn run_translate() -> Result<()> {
    println!("\n=== Translation (Qwen3-4B-Instruct Q4_K_M, Metal) ===");
    let llm = Llm::load()?;
    for s in EN_SENTENCES {
        let (out, wall, gen) = llm.translate(s, "English", "Japanese")?;
        println!("en→ja {wall:>5} ms {gen:>3} tok ({:.1} tok/s): {out}", gen as f64 / (wall as f64 / 1000.0));
    }
    for s in JA_SENTENCES {
        let (out, wall, gen) = llm.translate(s, "Japanese", "English")?;
        println!("ja→en {wall:>5} ms {gen:>3} tok ({:.1} tok/s): {out}", gen as f64 / (wall as f64 / 1000.0));
    }
    Ok(())
}

// ------------------------------------------------------------------- concurrent

#[cfg(all(feature = "asr", feature = "llm"))]
fn run_concurrent() -> Result<()> {
    println!("\n=== Concurrent (ASR windowed loop + translation) ===");
    let audio = read_wav_16k_mono(AUDIO_JA)?;

    let asr_thread = std::thread::spawn(move || -> Result<Vec<u128>> {
        let whisper = Whisper::load()?;
        let dur_s = audio.len() / SAMPLE_RATE;
        let mut walls = Vec::new();
        for end_s in (STEP_SECS..=dur_s).step_by(STEP_SECS) {
            let end = (end_s * SAMPLE_RATE).min(audio.len());
            let start = end.saturating_sub(WINDOW_SECS * SAMPLE_RATE);
            let (_, wall) = whisper.decode(&audio[start..end], "ja")?;
            walls.push(wall);
        }
        Ok(walls)
    });

    let llm = Llm::load()?;
    let mut tr_walls = Vec::new();
    for s in EN_SENTENCES.iter().cycle().take(6) {
        let (_, wall, _) = llm.translate(s, "English", "Japanese")?;
        tr_walls.push(wall);
    }

    let asr_walls = asr_thread.join().expect("asr thread")?;
    let avg = |v: &[u128]| v.iter().sum::<u128>() / v.len().max(1) as u128;
    println!(
        "ASR decode under load: avg {} ms, worst {} ms ({} windows)",
        avg(&asr_walls),
        asr_walls.iter().max().unwrap_or(&0),
        asr_walls.len()
    );
    println!(
        "translation under load: avg {} ms, worst {} ms ({} sentences)",
        avg(&tr_walls),
        tr_walls.iter().max().unwrap_or(&0),
        tr_walls.len()
    );
    Ok(())
}
