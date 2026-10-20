//! Immutable MLX Turbo weights and matching upstream tokenizer assets.
use super::ModelDefinition;

pub const MLX_REFERENCE: ModelDefinition = ModelDefinition {
    id: "large-v3-turbo-mlx-fp16",
    file: "weights.safetensors",
    bytes: 1_613_977_612,
    sha256: "951ed3fc1203e6a62467abb2144a96ce7eafca8fa77e3704fdb8635ff3e7f8a6",
};

pub(super) const SUPPORT: [ModelDefinition; 3] = [
    ModelDefinition {
        id: "mlx-turbo-fp16-config",
        file: "config.json",
        bytes: 268,
        sha256: "b34fc29e4e11e0a25e812775dd67f4dd16fc2c8eb43d28ae25ff7d660ecb6379",
    },
    ModelDefinition {
        id: "mlx-turbo-fp16-tokenizer",
        file: "tokenizer.json",
        bytes: 2_710_337,
        sha256: "297b13372ac43916285644fb9687add3cc62ee2a1adb60da3dc25cc94c1871fd",
    },
    ModelDefinition {
        id: "mlx-turbo-fp16-tokenizer-config",
        file: "tokenizer_config.json",
        bytes: 282_843,
        sha256: "844b642c73a91359722f47b35705f7174686df33d252695d8572cf9ac03a6389",
    },
];

pub(super) fn url(definition: ModelDefinition) -> Option<&'static str> {
    match definition.id {
        "large-v3-turbo-mlx-fp16" => Some(
            "https://huggingface.co/mlx-community/whisper-large-v3-turbo/resolve/a4aaeec0636e6fef84abdcbe3544cb2bf7e9f6fb/weights.safetensors",
        ),
        "mlx-turbo-fp16-config" => Some(
            "https://huggingface.co/mlx-community/whisper-large-v3-turbo/resolve/a4aaeec0636e6fef84abdcbe3544cb2bf7e9f6fb/config.json",
        ),
        "mlx-turbo-fp16-tokenizer" => Some(
            "https://huggingface.co/openai/whisper-large-v3-turbo/resolve/41f01f3fe87f28c78e2fbf8b568835947dd65ed9/tokenizer.json",
        ),
        "mlx-turbo-fp16-tokenizer-config" => Some(
            "https://huggingface.co/openai/whisper-large-v3-turbo/resolve/41f01f3fe87f28c78e2fbf8b568835947dd65ed9/tokenizer_config.json",
        ),
        _ => None,
    }
}

/// Local hash-verified derivation from the pinned GGML reference; no download
/// endpoint. Selectable with MLX support; setup installs the source first.
pub const MLX_REFERENCE_COEFFICIENTS: ModelDefinition = ModelDefinition {
    id: "large-v3-turbo-reference-q5-coefficients-mlx-f32",
    file: "weights.safetensors",
    bytes: 3_235_574_296,
    sha256: "277a2661c6d85f7e620adad07f7fda78b052a5a66b2c16e9fb6ccb3ac134d042",
};
