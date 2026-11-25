//! Sentence embeddings for categorizing skills: the bundled
//! all-MiniLM-L6-v2 run on CPU with candle (pure Rust — no native
//! runtime to ship per platform). Text in, unit-length 384-d vectors
//! out, so cosine similarity is a plain dot product.

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use std::path::Path;
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

/// Identifies the vectors this build produces; cached vectors from any
/// other model are discarded. Keep in sync with scripts/fetch-model.mjs.
pub const MODEL_ID: &str =
    "sentence-transformers/all-MiniLM-L6-v2@1110a243fdf4706b3f48f1d95db1a4f5529b4d41";

/// Skill text is a name plus a description or SKILL.md excerpt; past a
/// couple hundred tokens nothing changes which category wins.
const MAX_TOKENS: usize = 256;

pub struct Embedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl Embedder {
    /// Load from a directory holding `config.json`, `tokenizer.json`
    /// and `model.safetensors` (see scripts/fetch-model.mjs).
    pub fn load(dir: &Path) -> Result<Self, String> {
        let device = Device::Cpu;
        let config: Config =
            serde_json::from_str(&std::fs::read_to_string(dir.join("config.json")).map_err(err)?)
                .map_err(err)?;
        let mut tokenizer = Tokenizer::from_file(dir.join("tokenizer.json")).map_err(err)?;
        tokenizer.with_padding(Some(PaddingParams {
            strategy: PaddingStrategy::BatchLongest,
            ..Default::default()
        }));
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: MAX_TOKENS,
                ..Default::default()
            }))
            .map_err(err)?;
        let weights = std::fs::read(dir.join("model.safetensors")).map_err(err)?;
        let vb = VarBuilder::from_buffered_safetensors(weights, DTYPE, &device).map_err(err)?;
        let model = BertModel::load(vb, &config).map_err(err)?;
        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    /// Embed a batch: mean-pool the token states under the attention
    /// mask (how sentence-transformers trained this model), then
    /// L2-normalize.
    pub fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(err)?;
        let ids: Vec<Tensor> = encodings
            .iter()
            .map(|e| Tensor::new(e.get_ids(), &self.device))
            .collect::<Result<_, _>>()
            .map_err(err)?;
        let masks: Vec<Tensor> = encodings
            .iter()
            .map(|e| Tensor::new(e.get_attention_mask(), &self.device))
            .collect::<Result<_, _>>()
            .map_err(err)?;
        let ids = Tensor::stack(&ids, 0).map_err(err)?;
        let mask = Tensor::stack(&masks, 0).map_err(err)?;
        let token_types = ids.zeros_like().map_err(err)?;

        let hidden = self
            .model
            .forward(&ids, &token_types, Some(&mask))
            .map_err(err)?; // [batch, tokens, 384]
        let mask = mask
            .to_dtype(DType::F32)
            .map_err(err)?
            .unsqueeze(2)
            .map_err(err)?;
        let summed = hidden
            .broadcast_mul(&mask)
            .map_err(err)?
            .sum(1)
            .map_err(err)?;
        let counts = mask.sum(1).map_err(err)?;
        let mean = summed.broadcast_div(&counts).map_err(err)?;
        let norm = mean
            .sqr()
            .map_err(err)?
            .sum_keepdim(1)
            .map_err(err)?
            .sqrt()
            .map_err(err)?;
        mean.broadcast_div(&norm)
            .map_err(err)?
            .to_vec2::<f32>()
            .map_err(err)
    }
}

/// Little-endian f32 bytes for a SQLite BLOB.
pub fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

// `as_chunks` would be tidier but needs Rust 1.88; the MSRV is 1.77.
#[allow(clippy::manual_slice_size_calculation, unknown_lints, clippy::chunks_exact_with_const_size)]
pub fn from_blob(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blobs_round_trip() {
        let v = vec![0.5, -1.25, 3.0];
        assert_eq!(from_blob(&to_blob(&v)), v);
    }

    /// Runs against the real bundled model when it has been fetched
    /// (`npm run fetch-model`); skipped otherwise so CI without the
    /// model still passes.
    #[test]
    fn real_model_ranks_related_text_higher() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/minilm");
        if !dir.join("model.safetensors").is_file() {
            eprintln!("model not fetched; skipping");
            return;
        }
        let e = Embedder::load(&dir).unwrap();
        let v = e
            .embed(&[
                "pdf: extract text and tables from PDF files".into(),
                "Document skills: reading and creating PDFs, Word documents and spreadsheets"
                    .into(),
                "Career: writing resumes and preparing for job interviews".into(),
            ])
            .unwrap();
        assert_eq!(v[0].len(), 384);
        assert!((dot(&v[0], &v[0]) - 1.0).abs() < 1e-4, "unit length");
        assert!(dot(&v[0], &v[1]) > dot(&v[0], &v[2]));
    }
}
