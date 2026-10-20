//! One-time exact conversion of the SHA-pinned reference. No inference path.
//! Q5_0 layout follows the pinned ggml-quants.c dequantize_row_q5_0. The entire
//! derived output must also match its independent pre-existing SHA before rename.
use super::{
    MLX_REFERENCE_COEFFICIENTS, ModelError, REFERENCE, VerifiedFile, verify_file_cancellable,
};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
use tokio_util::sync::CancellationToken;
struct Tensor {
    name: String,
    shape: Vec<usize>,
    count: usize,
    kind: i32,
    offset: u64,
    bytes: usize,
}
fn invalid() -> ModelError {
    ModelError::Hash
}
fn check_cancel(cancel: &CancellationToken) -> Result<(), ModelError> {
    if cancel.is_cancelled() {
        Err(ModelError::Cancelled)
    } else {
        Ok(())
    }
}
fn i32_le(f: &mut File) -> Result<i32, ModelError> {
    let mut b = [0; 4];
    f.read_exact(&mut b)?;
    Ok(i32::from_le_bytes(b))
}
fn zeros<T: Default + Clone>(n: usize) -> Result<Vec<T>, ModelError> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
    v.resize(n, T::default());
    Ok(v)
}
fn half(bits: u16) -> f32 {
    let sign = u32::from(bits & 0x8000) << 16;
    let e = (bits >> 10) & 31;
    let m = bits & 1023;
    if e == 0 {
        return f32::from_bits((f32::from(m) * 2.0_f32.powi(-24)).to_bits() | sign);
    }
    f32::from_bits(
        sign | ((if e == 31 { 255 } else { u32::from(e) + 112 }) << 23) | (u32::from(m) << 13),
    )
}
fn parse(f: &mut File, cancel: &CancellationToken) -> Result<Vec<Tensor>, ModelError> {
    if i32_le(f)? != 0x67676d6c {
        return Err(invalid());
    }
    for expected in [51866, 1500, 1280, 20, 32, 448, 1280, 20, 4, 128, 2008] {
        if i32_le(f)? != expected {
            return Err(invalid());
        }
    }
    if i32_le(f)? != 128 || i32_le(f)? != 201 {
        return Err(invalid());
    }
    f.seek(SeekFrom::Current(128 * 201 * 4))?;
    if i32_le(f)? != 50257 {
        return Err(invalid());
    }
    for _ in 0..50257 {
        check_cancel(cancel)?;
        let n = i32_le(f)?;
        if !(0..65536).contains(&n) {
            return Err(invalid());
        }
        f.seek(SeekFrom::Current(i64::from(n)))?;
    }
    let mut tensors = Vec::new();
    let mut names = BTreeSet::new();
    let mut total = 0_usize;
    while f.stream_position()? < REFERENCE.bytes() {
        check_cancel(cancel)?;
        let rank = i32_le(f)?;
        let name_len = i32_le(f)?;
        let kind = i32_le(f)?;
        if !(1..=3).contains(&rank) || !(1..=256).contains(&name_len) || ![0, 1, 6].contains(&kind)
        {
            return Err(invalid());
        }
        let mut shape = Vec::new();
        let mut count = 1_usize;
        for _ in 0..rank {
            let d = i32_le(f)?;
            if !(1..=100000).contains(&d) {
                return Err(invalid());
            }
            count = count
                .checked_mul(d as usize)
                .filter(|n| *n <= 70000000)
                .ok_or_else(invalid)?;
            shape.push(d as usize);
        }
        if kind == 6 && !shape[0].is_multiple_of(32) {
            return Err(invalid());
        }
        let mut name = zeros::<u8>(name_len as usize)?;
        f.read_exact(&mut name)?;
        if !name
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._".contains(c))
        {
            return Err(invalid());
        }
        let name = String::from_utf8(name)
            .map_err(|_| invalid())?
            .replace(".mlp.0.", ".mlp1.")
            .replace(".mlp.2.", ".mlp2.");
        if !names.insert(name.clone()) {
            return Err(invalid());
        }
        shape.reverse();
        if name == "encoder.conv1.weight" || name == "encoder.conv2.weight" {
            if shape.len() != 3 {
                return Err(invalid());
            }
            shape.swap(1, 2);
        }
        if name == "encoder.conv1.bias" || name == "encoder.conv2.bias" {
            if count != 1280 {
                return Err(invalid());
            }
            shape = vec![1280];
        }
        let offset = f.stream_position()?;
        let bytes = if kind == 6 {
            count / 32 * 22
        } else {
            count * if kind == 1 { 2 } else { 4 }
        };
        if offset + bytes as u64 > REFERENCE.bytes() {
            return Err(invalid());
        }
        f.seek(SeekFrom::Start(offset + bytes as u64))?;
        tensors.push(Tensor {
            name,
            shape,
            count,
            kind,
            offset,
            bytes,
        });
        total = total.checked_add(count).ok_or_else(invalid)?;
    }
    if tensors.len() != 587 || total != 808878080 || f.stream_position()? != REFERENCE.bytes() {
        return Err(invalid());
    }
    Ok(tensors)
}
fn header(tensors: &[Tensor]) -> Result<Vec<u8>, ModelError> {
    let mut text = format!(
        "{{\"__metadata__\":{{\"source\":\"verified GGML Q5_0 reference; exact ggml dequantization to F32\",\"source_sha256\":\"{}\"}}",
        REFERENCE.sha256()
    );
    let mut cursor = 0_usize;
    for t in tensors {
        let shape = serde_json::to_string(&t.shape).map_err(|_| invalid())?;
        text.push_str(&format!(
            ",\"{}\":{{\"dtype\":\"F32\",\"shape\":{},\"data_offsets\":[{},{}]}}",
            t.name,
            shape,
            cursor,
            cursor + t.count * 4
        ));
        cursor += t.count * 4;
    }
    text.push('}');
    while !text.len().is_multiple_of(8) {
        text.push(' ');
    }
    Ok(text.into_bytes())
}
fn decode(t: &Tensor, raw: &[u8], cancel: &CancellationToken) -> Result<Vec<f32>, ModelError> {
    let mut values = zeros::<f32>(t.count)?;
    match t.kind {
        0 => {
            for (out, b) in values.iter_mut().zip(raw.as_chunks::<4>().0.iter()) {
                *out = f32::from_le_bytes(*b);
            }
        }
        1 => {
            for (out, b) in values.iter_mut().zip(raw.as_chunks::<2>().0.iter()) {
                *out = half(u16::from_le_bytes([b[0], b[1]]));
            }
        }
        6 => {
            for (index, b) in raw.as_chunks::<22>().0.iter().enumerate() {
                if index.is_multiple_of(8192) {
                    check_cancel(cancel)?;
                }
                let d = half(u16::from_le_bytes([b[0], b[1]]));
                let qh = u32::from_le_bytes(b[2..6].try_into().map_err(|_| invalid())?);
                for j in 0..16 {
                    let x0 = i32::from(b[6 + j] & 15) | ((((qh >> j) & 1) as i32) * 16);
                    let x1 = i32::from(b[6 + j] >> 4) | ((((qh >> (j + 16)) & 1) as i32) * 16);
                    values[index * 32 + j] = (x0 - 16) as f32 * d;
                    values[index * 32 + j + 16] = (x1 - 16) as f32 * d;
                }
            }
        }
        _ => return Err(invalid()),
    }
    if values.iter().any(|x| !x.is_finite()) {
        return Err(invalid());
    }
    if t.name == "encoder.conv1.weight" || t.name == "encoder.conv2.weight" {
        let mut permuted = zeros::<f32>(t.count)?;
        let [outputs, kernel, channels] = t.shape.as_slice() else {
            return Err(invalid());
        };
        for o in 0..*outputs {
            check_cancel(cancel)?;
            for k in 0..*kernel {
                for c in 0..*channels {
                    permuted[(o * kernel + k) * channels + c] =
                        values[(o * channels + c) * kernel + k];
                }
            }
        }
        values = permuted;
    }
    Ok(values)
}
pub(super) fn convert(
    directory: &Path,
    cancel: &CancellationToken,
    progress: &mut impl FnMut(u64),
) -> Result<PathBuf, ModelError> {
    check_cancel(cancel)?;
    let source = directory.join(REFERENCE.file_name());
    verify_file_cancellable(&source, REFERENCE, cancel)?;
    let mut file = File::open(source)?;
    let tensors = parse(&mut file, cancel)?;
    let header = header(&tensors)?;
    let mut output = VerifiedFile::new(directory, MLX_REFERENCE_COEFFICIENTS)?;
    progress(0);
    output.write(&(header.len() as u64).to_le_bytes())?;
    output.write(&header)?;
    let mut staging = [0_u8; 65536];
    for t in tensors {
        check_cancel(cancel)?;
        file.seek(SeekFrom::Start(t.offset))?;
        let mut raw = zeros::<u8>(t.bytes)?;
        file.read_exact(&mut raw)?;
        let values = decode(&t, &raw, cancel)?;
        drop(raw);
        for chunk in values.chunks(16384) {
            check_cancel(cancel)?;
            for (n, v) in chunk.iter().enumerate() {
                staging[n * 4..n * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
            output.write(&staging[..chunk.len() * 4])?;
            progress(output.received());
        }
    }
    check_cancel(cancel)?;
    output.finish()
}
