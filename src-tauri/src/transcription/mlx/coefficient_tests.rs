//! Public model-loading/encoding contract for an explicit checkpoint table.
use super::*;
use mlxrs::audio::stt::model::AutoregressiveStt;
use std::collections::HashMap;

type Result<T> = std::result::Result<T, String>;

pub(super) fn weights(positional_shape: &[i32]) -> Result<HashMap<String, Array>> {
    let mut tensors = HashMap::new();
    let mut put = |name: String, shape: &[i32], ones: bool| -> Result<()> {
        let count = shape.iter().map(|n| *n as usize).product();
        tensors.insert(
            name,
            Array::from_slice(&vec![if ones { 1.0_f32 } else { 0.0 }; count], &shape)
                .map_err(|e| e.to_string())?,
        );
        Ok(())
    };
    for prefix in ["encoder.conv1", "encoder.conv2"] {
        put(format!("{prefix}.weight"), &[4, 3, 4], false)?;
        put(format!("{prefix}.bias"), &[4], false)?;
    }
    for stack in ["encoder", "decoder"] {
        let block = format!("{stack}.blocks.0");
        let mut attentions = vec!["attn"];
        if stack == "decoder" {
            attentions.push("cross_attn");
        }
        for attention in attentions {
            for projection in ["query", "key", "value", "out"] {
                put(
                    format!("{block}.{attention}.{projection}.weight"),
                    &[4, 4],
                    false,
                )?;
                if projection != "key" {
                    put(
                        format!("{block}.{attention}.{projection}.bias"),
                        &[4],
                        false,
                    )?;
                }
            }
            put(format!("{block}.{attention}_ln.weight"), &[4], true)?;
            put(format!("{block}.{attention}_ln.bias"), &[4], false)?;
        }
        put(format!("{block}.mlp_ln.weight"), &[4], true)?;
        put(format!("{block}.mlp_ln.bias"), &[4], false)?;
        put(format!("{block}.mlp1.weight"), &[16, 4], false)?;
        put(format!("{block}.mlp1.bias"), &[16], false)?;
        put(format!("{block}.mlp2.weight"), &[4, 16], false)?;
        put(format!("{block}.mlp2.bias"), &[4], false)?;
    }
    for prefix in ["encoder.ln_post", "decoder.ln"] {
        put(format!("{prefix}.weight"), &[4], true)?;
        put(format!("{prefix}.bias"), &[4], false)?;
    }
    put("decoder.token_embedding.weight".into(), &[8, 4], false)?;
    put("decoder.positional_embedding".into(), &[6, 4], false)?;
    let count = positional_shape.iter().map(|n| *n as usize).product();
    let values: Vec<_> = (0..count).map(|i| (i % 4) as f32).collect();
    tensors.insert(
        "encoder.positional_embedding".into(),
        Array::from_slice(&values, &positional_shape).map_err(|e| e.to_string())?,
    );
    Ok(tensors)
}

#[test]
fn explicit_encoder_checkpoint_table_controls_public_encode_output() -> Result<()> {
    std::thread::spawn(|| {
        let result = (|| {
            let dims =
                ModelDimensions::new(4, 1500, 4, 2, 1, 8, 6, 4, 2, 1).map_err(|e| e.to_string())?;
            let model = WhisperModel::from_weights(dims, weights(&[1500, 4])?, Dtype::F32)
                .map_err(|e| e.to_string())?;
            let mel = Array::zeros::<f32>(&(3000_usize, 4_usize)).map_err(|e| e.to_string())?;
            let mut encoded = model.encode(&mel).map_err(|e| e.to_string())?;
            let actual = encoded.to_vec::<f32>().map_err(|e| e.to_string())?;
            // Zero convolutions/attention/MLP leave the supplied [0,1,2,3]
            // row. Final LayerNorm(eps1e-5) has mean1.5, variance1.25.
            let expected = [-1.3416354_f32, -0.4472118, 0.4472118, 1.3416354];
            if actual.len() != 6000
                || actual
                    .iter()
                    .enumerate()
                    .any(|(i, value)| (value - expected[i % 4]).abs() > 0.00001)
            {
                return Err(format!(
                    "explicit checkpoint encoder table was not consumed; first row {:?}",
                    &actual[..actual.len().min(4)]
                ));
            }
            Ok(())
        })();
        result.and(cleanup_streams().map_err(|e| e.to_string()))
    })
    .join()
    .map_err(|_| "encoder coefficient control worker panicked".to_owned())?
}

#[test]
fn malformed_encoder_checkpoint_table_is_rejected_before_encoding() -> Result<()> {
    std::thread::spawn(|| {
        let result = (|| {
            let dims =
                ModelDimensions::new(4, 1500, 4, 2, 1, 8, 6, 4, 2, 1).map_err(|e| e.to_string())?;
            match WhisperModel::from_weights(dims, weights(&[1499, 4])?, Dtype::F32) {
                Err(error) if error.to_string().contains("encoder.positional_embedding") => Ok(()),
                Err(error) => Err(format!("wrong error for malformed position table: {error}")),
                Ok(_) => Err("malformed checkpoint position table was silently ignored".into()),
            }
        })();
        result.and(cleanup_streams().map_err(|e| e.to_string()))
    })
    .join()
    .map_err(|_| "malformed position table worker panicked".to_owned())?
}
