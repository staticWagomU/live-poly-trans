//! Translation backend isolated in a cdylib.
//!
//! whisper-rs and llama-cpp-2 each vendor their own incompatible ggml;
//! statically linking both into one image merges the C symbols and
//! crashes at runtime (docs/step0-results.md). A cdylib carries a private
//! copy of ggml — macOS two-level namespaces (and per-DLL resolution on
//! Windows) keep it separate from the host's — so the app remains a
//! single process (ADR-153800).
//!
//! C ABI: `lpt_translate_init` once, `lpt_translate` per sentence
//! (blocking; call from a queue thread), `lpt_translate_free` per result,
//! `lpt_translate_shutdown` at the end.

use std::ffi::{c_char, CStr, CString};
use std::num::NonZeroU32;

use anyhow::{Context, Result};
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::{AddBos, LlamaModel};

pub struct Translator {
    backend: LlamaBackend,
    model: LlamaModel,
}

/// # Safety
/// `model_path` must be a valid NUL-terminated UTF-8 path.
#[no_mangle]
pub unsafe extern "C" fn lpt_translate_init(
    model_path: *const c_char,
    n_gpu_layers: u32,
) -> *mut Translator {
    let path = CStr::from_ptr(model_path).to_string_lossy().into_owned();
    match init(&path, n_gpu_layers) {
        Ok(t) => Box::into_raw(Box::new(t)),
        Err(e) => {
            eprintln!("lpt-translate init: {e:#}");
            std::ptr::null_mut()
        }
    }
}

fn init(path: &str, n_gpu_layers: u32) -> Result<Translator> {
    let backend = LlamaBackend::init()?;
    let params =
        llama_cpp_2::model::params::LlamaModelParams::default().with_n_gpu_layers(n_gpu_layers);
    let model = LlamaModel::load_from_file(&backend, path, &params)
        .with_context(|| format!("load llm model {path}"))?;
    Ok(Translator { backend, model })
}

/// Translate one sentence. Returns a heap CString (free with
/// [`lpt_translate_free`]) or null on error.
///
/// # Safety
/// `handle` must come from [`lpt_translate_init`]; the strings must be
/// valid NUL-terminated UTF-8. Not thread-safe per handle: serialize calls.
#[no_mangle]
pub unsafe extern "C" fn lpt_translate(
    handle: *mut Translator,
    sentence: *const c_char,
    source_lang: *const c_char,
    target_lang: *const c_char,
) -> *mut c_char {
    let Some(translator) = handle.as_mut() else {
        return std::ptr::null_mut();
    };
    let sentence = CStr::from_ptr(sentence).to_string_lossy();
    let source = CStr::from_ptr(source_lang).to_string_lossy();
    let target = CStr::from_ptr(target_lang).to_string_lossy();
    match translate(translator, &sentence, &source, &target) {
        Ok(out) => CString::new(out).map_or(std::ptr::null_mut(), CString::into_raw),
        Err(e) => {
            eprintln!("lpt-translate: {e:#}");
            std::ptr::null_mut()
        }
    }
}

/// # Safety
/// `s` must be a pointer returned by [`lpt_translate`] (or null).
#[no_mangle]
pub unsafe extern "C" fn lpt_translate_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// # Safety
/// `handle` must come from [`lpt_translate_init`] (or be null) and must
/// not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn lpt_translate_shutdown(handle: *mut Translator) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

fn translate(t: &mut Translator, sentence: &str, source: &str, target: &str) -> Result<String> {
    let prompt = format!(
        "<|im_start|>system\nYou are a professional simultaneous interpreter. \
         Translate the user's sentence from {source} to {target}. \
         Output only the translation, nothing else.<|im_end|>\n\
         <|im_start|>user\n{sentence}<|im_end|>\n<|im_start|>assistant\n"
    );

    let ctx_params = llama_cpp_2::context::params::LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(2048));
    let mut ctx = t.model.new_context(&t.backend, ctx_params)?;

    let tokens = t.model.str_to_token(&prompt, AddBos::Never)?;
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
    let mut generated = 0u32;
    loop {
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        sampler.accept(token);
        if t.model.is_eog_token(token) || generated >= 256 {
            break;
        }
        out_bytes.extend(t.model.token_to_piece_bytes(token, 64, false, None)?);
        generated += 1;
        batch.clear();
        batch.add(token, pos, &[0], true)?;
        ctx.decode(&mut batch)?;
        pos += 1;
    }
    Ok(String::from_utf8_lossy(&out_bytes).trim().to_string())
}
