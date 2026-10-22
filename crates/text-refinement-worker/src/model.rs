use crate::{Generator, manifest, punctuation, selection};
use dictation_core::refinement::{
    EditKind, MAX_ORIGINAL_BYTES, OriginalTranscript, StagedEdits, proposal_candidates, validate,
    validate_staged,
};
use mlxrs::{
    lm::{
        factory::{ModelConfiguration, ModelContext, ModelTypeRegistry},
        generate::{FinishReason, GenConfig},
        models::qwen3::{Qwen3, Qwen3Config},
        structured::build_json_schema_logits_processor,
    },
    memory::CacheLimitGuard,
    tokenizer::Tokenizer,
};
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use text_refinement_protocol::{FailureKind, Language};

pub use text_refinement_protocol::{
    TEXT_POLICY_VERSION as POLICY_VERSION, TEXT_PROMPT_VERSION as PROMPT_VERSION,
};
const PROMPT: &str = include_str!("prompt.txt");
const MAX_OUTPUT_BYTES: usize = 32 * 1024;
const MAX_INPUT_TOKENS: usize = 8192;
const JOB_TIMEOUT: Duration = Duration::from_secs(30);

struct OutputBudget {
    bytes: usize,
    tokens: usize,
}

pub struct MlxGenerator {
    // Field order also drops model arrays before the allocator guard on errors.
    context: Option<ModelContext>,
    cache: Option<CacheLimitGuard>,
    _snapshot: manifest::VerifiedSnapshot,
}

impl MlxGenerator {
    pub fn load(
        directory: &Path,
        owner_directory: &Path,
        cancel: &AtomicBool,
    ) -> Result<Self, FailureKind> {
        let snapshot = manifest::snapshot(directory, owner_directory, cancel)?;
        let directory = snapshot.path();
        let cache =
            CacheLimitGuard::install(256 * 1024 * 1024).map_err(|_| FailureKind::ModelLoad)?;
        let registry = ModelTypeRegistry::new().with(
            "qwen3",
            Box::new(|loaded| {
                let config = Qwen3Config::from_json(&loaded.config_json)?;
                let weights = loaded
                    .weights
                    .iter()
                    .map(|(key, value)| Ok((key.clone(), value.try_clone()?)))
                    .collect::<mlxrs::error::Result<std::collections::HashMap<_, _>>>()?;
                Ok(Box::new(Qwen3::from_weights(config, weights)?))
            }),
        );
        let loaded = ModelContext::load(&ModelConfiguration::from_directory(directory), &registry)
            .map_err(|_| FailureKind::ModelLoad)?;
        let (model, tokenizer, config) = loaded.into_parts();
        let base = serde_json::to_vec(tokenizer.config()).map_err(|_| FailureKind::ModelLoad)?;
        let template = manifest::read_template(directory)?;
        let adapted = manifest::adapted_config(&base, &template)?;
        manifest::verify_adapter(&adapted, &snapshot.manifest)?;
        let tokenizer_config =
            serde_json::from_slice(&adapted).map_err(|_| FailureKind::ModelLoad)?;
        let mut eos = Vec::new();
        if let Some(primary) = tokenizer.eos_token_id() {
            eos.push(primary);
        }
        for id in tokenizer.eos_token_ids_iter() {
            if !eos.contains(&id) {
                eos.push(id);
            }
        }
        let adapted_tokenizer = Tokenizer::from_loaded(
            tokenizer.hf().clone(),
            tokenizer_config,
            tokenizer.detokenizer_class(),
            Some(&eos),
        )
        .map_err(|_| FailureKind::ModelLoad)?;
        if cancel.load(Ordering::Acquire) {
            return Err(FailureKind::Incomplete);
        }
        Ok(Self {
            context: Some(ModelContext::new(model, adapted_tokenizer, config)),
            cache: Some(cache),
            _snapshot: snapshot,
        })
    }
    fn infer(
        &self,
        prompt: &str,
        content: &serde_json::Value,
        schema: serde_json::Value,
        cancel: &AtomicBool,
        started: Instant,
        remaining: &mut OutputBudget,
    ) -> Result<String, FailureKind> {
        let context = self.context.as_ref().ok_or(FailureKind::Generation)?;
        let content = serde_json::to_string(content)
            .map_err(|_| FailureKind::Generation)?
            .replace('<', "\\u003c")
            .replace('>', "\\u003e");
        let messages = serde_json::json!([{"role":"system","content":prompt},{"role":"user","content":content}]);
        let input = context
            .apply_chat_template_ids(&messages, None, true, false, None)
            .map_err(|_| FailureKind::Generation)?;
        if input.len() > MAX_INPUT_TOKENS {
            return Err(FailureKind::InputLimit);
        }
        if remaining.tokens == 0 {
            return Err(FailureKind::InputLimit);
        }
        let mut parameters = GenConfig::new()
            .with_max_tokens(remaining.tokens)
            .with_prefill_step_size(256);
        parameters.temp = 0.;
        let grammar = build_json_schema_logits_processor(
            schema,
            context.tokenizer(),
            Some(context.config().vocab_size as usize),
        )
        .map_err(|_| FailureKind::Generation)?;
        if cancel.load(Ordering::Acquire) {
            return Err(FailureKind::Incomplete);
        }
        if started.elapsed() > JOB_TIMEOUT {
            return Err(FailureKind::Timeout);
        }
        let mut stream = context.stream_generate_with_processors(
            &input,
            parameters,
            vec![grammar.into_logits_processor()],
        );
        let mut output = String::new();
        let mut stopped = false;
        let mut used_tokens = 0;
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err(FailureKind::Incomplete);
            }
            if started.elapsed() > JOB_TIMEOUT {
                return Err(FailureKind::Timeout);
            }
            let Some(step) = stream.next() else {
                break;
            };
            let step = step.map_err(|_| FailureKind::Generation)?;
            if cancel.load(Ordering::Acquire) {
                return Err(FailureKind::Incomplete);
            }
            if started.elapsed() > JOB_TIMEOUT {
                return Err(FailureKind::Timeout);
            }
            if step.text.len() > remaining.bytes {
                return Err(FailureKind::InputLimit);
            }
            remaining.bytes -= step.text.len();
            used_tokens = step.generation_tokens;
            output.push_str(&step.text);
            if let Some(reason) = step.finish_reason {
                stopped = matches!(reason, FinishReason::Eos);
                if !stopped {
                    return Err(FailureKind::Incomplete);
                }
            }
        }
        drop(stream);
        if !stopped {
            return Err(FailureKind::Incomplete);
        }
        remaining.tokens = remaining
            .tokens
            .checked_sub(used_tokens)
            .ok_or(FailureKind::InputLimit)?;
        Ok(output)
    }
}

impl Generator for MlxGenerator {
    fn generate(
        &mut self,
        text: &str,
        language: Language,
        cancel: &AtomicBool,
    ) -> Result<StagedEdits, FailureKind> {
        if text.len() > MAX_ORIGINAL_BYTES {
            return Err(FailureKind::InputLimit);
        }
        if cancel.load(Ordering::Acquire) {
            return Err(FailureKind::Incomplete);
        }
        let started = Instant::now();
        let mut remaining = OutputBudget {
            bytes: MAX_OUTPUT_BYTES,
            tokens: 4096,
        };
        let original = OriginalTranscript::new(text.into()).map_err(|_| FailureKind::InputLimit)?;
        let mut choices =
            proposal_candidates(&original, language.into()).map_err(|_| FailureKind::InputLimit)?;
        choices.retain(|edit| edit.kind != EditKind::PunctuationCase);
        let structural = if choices.is_empty() {
            Vec::new()
        } else {
            let options = selection::prompt_options(&original, &choices)?;
            let output = self.infer(
                PROMPT,
                &serde_json::json!({"language":language,"dictation":text,"choices":options}),
                selection::output_schema(choices.len())?,
                cancel,
                started,
                &mut remaining,
            )?;
            selection::decode(&output, &original, &choices, language.into())?
        };
        let interim = validate(&original, &structural, language.into())
            .map_err(|_| FailureKind::InvalidCandidate)?;
        let plan = punctuation::Plan::new(
            OriginalTranscript::new(interim.as_str().into())
                .map_err(|_| FailureKind::InputLimit)?,
        )?;
        let output = self.infer(
            include_str!("punctuation-prompt.txt"),
            &serde_json::json!({"language":language,"dictation":interim.as_str()}),
            plan.schema(),
            cancel,
            started,
            &mut remaining,
        )?;
        let punctuation = plan.decode(&output, language.into())?;
        let proposal = StagedEdits {
            structural,
            punctuation,
        };
        validate_staged(&original, &proposal, language.into())
            .map_err(|_| FailureKind::InvalidCandidate)?;
        if cancel.load(Ordering::Acquire) {
            return Err(FailureKind::Incomplete);
        }
        if started.elapsed() > JOB_TIMEOUT {
            return Err(FailureKind::Timeout);
        }
        Ok(proposal)
    }

    fn finish(mut self) -> Result<(), FailureKind> {
        drop(self.context.take());
        let streams = mlxrs::stream::Stream::clear_current_thread_streams()
            .map_err(|_| FailureKind::Generation);
        let cache = self
            .cache
            .take()
            .ok_or(FailureKind::Generation)?
            .finish()
            .map_err(|_| FailureKind::Generation);
        streams.and(cache)
    }
}
