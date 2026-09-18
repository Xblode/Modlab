//! llama.cpp inference backend — feature `local-llm`, choice recorded in
//! ADR 0004. The API usage mirrors upstream's `examples/simple`
//! (utilityai/llama-cpp-rs), which is the reference for this fast-moving
//! crate's API.
//!
//! Deliberately minimal: the model is loaded per call, CPU-only, no grammar
//! constraint yet. The §6 lifecycle (lazy-load, idle-unload, queue while a
//! game runs — the host already refuses to call this during a session) and
//! GBNF-constrained output arrive with the MCP wiring.

use std::num::NonZeroU32;
use std::path::Path;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::AddBos;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::sampling::LlamaSampler;

/// Load the GGUF at `model_path`, tokenize `prompt`, decode, sample, and
/// return the generated text. Loads the model per call — slow by design for
/// now, so a load failure is reported instead of cached.
pub fn generate(model_path: &Path, prompt: &str, max_tokens: i32) -> Result<String, String> {
    let backend = LlamaBackend::init().map_err(|e| format!("init llama.cpp : {e}"))?;
    let model = LlamaModel::load_from_file(&backend, model_path, &LlamaModelParams::default())
        .map_err(|e| format!("chargement du modèle : {e}"))?;

    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(Some(NonZeroU32::new(2048).expect("2048 is non-zero")));
    let mut ctx = model
        .new_context(&backend, ctx_params)
        .map_err(|e| format!("création du contexte : {e}"))?;

    let tokens = model
        .str_to_token(prompt, AddBos::Always)
        .map_err(|e| format!("tokenization du prompt : {e}"))?;

    // Prompt pass: logits only for the last token (what we sample first).
    let mut batch = LlamaBatch::new(512, 1);
    let last_index: i32 = (tokens.len() - 1) as i32;
    for (i, token) in (0_i32..).zip(tokens.into_iter()) {
        let is_last = i == last_index;
        batch
            .add(token, i, &[0], is_last)
            .map_err(|e| format!("préparation du batch : {e}"))?;
    }
    ctx.decode(&mut batch)
        .map_err(|e| format!("décodage du prompt : {e}"))?;

    let mut sampler =
        LlamaSampler::chain_simple([LlamaSampler::dist(1234), LlamaSampler::greedy()]);

    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut out = String::new();
    let mut n_cur = batch.n_tokens();

    while n_cur <= max_tokens {
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        sampler.accept(token);
        if model.is_eog_token(token) {
            break;
        }
        let piece = model
            .token_to_piece(token, &mut decoder, true, None)
            .map_err(|e| format!("détokenization : {e}"))?;
        out.push_str(&piece);

        batch.clear();
        batch
            .add(token, n_cur, &[0], true)
            .map_err(|e| format!("préparation du batch : {e}"))?;
        n_cur += 1;
        ctx.decode(&mut batch)
            .map_err(|e| format!("génération : {e}"))?;
    }

    Ok(out)
}
