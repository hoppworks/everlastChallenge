//! Bounded developer replay inputs; never part of the application IPC.
use dictation_core::audio::MAX_SAMPLES;
use everlast_dictation::transcription::Language;
use std::{
    ffi::OsString,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

pub const MAX_ARGUMENTS: usize = 8;
pub const MAX_REPEATS: u32 = 30;
const MAX_PATH_BYTES: usize = 4096;
pub const MAX_REFERENCE_BYTES: usize = 256 * 1024;
const MAX_WAV_BYTES: u64 = (MAX_SAMPLES as u64) * 2 + 65_536;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum InputError {
    #[error(
        "usage: transcribe_probe MODEL WAV [--language de|en [--repeat 1..30] [--reference UTF8_FILE]]"
    )]
    Arguments,
    #[error("Replay language must be de or en")]
    Language,
    #[error("Replay repetitions must be between 1 and 30")]
    Repeats,
    #[error("The replay WAV could not be read as a regular file")]
    FixtureIo,
    #[error("Replay WAV must be valid PCM16 mono at 16000 Hz")]
    FixtureFormat,
    #[error("Replay WAV must contain 1 sample to 300 seconds and fit the bounded file-size limit")]
    FixtureLength,
    #[error("The reference could not be read as a regular file")]
    ReferenceIo,
    #[error("The reference must be UTF-8 text")]
    ReferenceUtf8,
    #[error("The reference must contain non-whitespace text")]
    ReferenceEmpty,
    #[error("The reference exceeds 256 KiB")]
    ReferenceLength,
}

pub struct Options {
    pub model: PathBuf,
    pub wav: PathBuf,
    pub language: Language,
    pub repeats: u32,
    pub reference: Option<PathBuf>,
    pub legacy: bool,
}

fn path(value: &std::ffi::OsStr) -> Result<PathBuf, InputError> {
    if value.is_empty() || value.as_encoded_bytes().len() > MAX_PATH_BYTES {
        return Err(InputError::Arguments);
    }
    Ok(value.into())
}

impl Options {
    pub fn parse(arguments: &[OsString]) -> Result<Self, InputError> {
        if arguments.len() < 2
            || arguments.len() > MAX_ARGUMENTS
            || !arguments.len().is_multiple_of(2)
        {
            return Err(InputError::Arguments);
        }
        let mut result = Self {
            model: path(&arguments[0])?,
            wav: path(&arguments[1])?,
            language: Language::English,
            repeats: 2,
            reference: None,
            legacy: arguments.len() == 2,
        };
        if result.legacy {
            return Ok(result);
        }
        let mut language = false;
        let mut repeats = false;
        for pair in arguments[2..].as_chunks::<2>().0 {
            match pair[0].to_str() {
                Some("--language") if !language => {
                    result.language = match pair[1].to_str() {
                        Some("de") => Language::German,
                        Some("en") => Language::English,
                        _ => return Err(InputError::Language),
                    };
                    language = true;
                }
                Some("--repeat") if !repeats => {
                    let count = pair[1]
                        .to_str()
                        .and_then(|value| value.parse::<u32>().ok())
                        .filter(|value| (1..=MAX_REPEATS).contains(value))
                        .ok_or(InputError::Repeats)?;
                    result.repeats = count;
                    repeats = true;
                }
                Some("--reference") if result.reference.is_none() => {
                    result.reference = Some(path(&pair[1])?)
                }
                _ => return Err(InputError::Arguments),
            }
        }
        if !language {
            return Err(InputError::Arguments);
        }
        Ok(result)
    }
}

pub fn load_audio(path: &Path) -> Result<Vec<f32>, InputError> {
    let metadata = fs::metadata(path).map_err(|_| InputError::FixtureIo)?;
    if !metadata.is_file() {
        return Err(InputError::FixtureIo);
    }
    if metadata.len() > MAX_WAV_BYTES {
        return Err(InputError::FixtureLength);
    }
    let file = File::open(path).map_err(|_| InputError::FixtureIo)?;
    let mut reader =
        hound::WavReader::new(file.take(MAX_WAV_BYTES)).map_err(|_| InputError::FixtureFormat)?;
    let spec = reader.spec();
    if spec.channels != 1
        || spec.sample_rate != 16_000
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
    {
        return Err(InputError::FixtureFormat);
    }
    let count = reader.duration() as usize;
    if count == 0 || count > MAX_SAMPLES {
        return Err(InputError::FixtureLength);
    }
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(count)
        .map_err(|_| InputError::FixtureLength)?;
    for sample in reader.samples::<i16>().take(MAX_SAMPLES + 1) {
        if samples.len() == MAX_SAMPLES {
            return Err(InputError::FixtureLength);
        }
        let value = sample.map_err(|_| InputError::FixtureFormat)?;
        samples.push(f32::from(value) / 32_768.0);
    }
    if samples.len() != count {
        return Err(InputError::FixtureFormat);
    }
    Ok(samples)
}

pub fn load_reference(path: &Path) -> Result<String, InputError> {
    let metadata = fs::metadata(path).map_err(|_| InputError::ReferenceIo)?;
    if !metadata.is_file() {
        return Err(InputError::ReferenceIo);
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| InputError::ReferenceIo)?
        .take((MAX_REFERENCE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| InputError::ReferenceIo)?;
    if bytes.len() > MAX_REFERENCE_BYTES {
        return Err(InputError::ReferenceLength);
    }
    let reference = String::from_utf8(bytes).map_err(|_| InputError::ReferenceUtf8)?;
    if reference.trim().is_empty() {
        return Err(InputError::ReferenceEmpty);
    }
    Ok(reference)
}

/// Preserve case and punctuation; only runs of Unicode whitespace are equal.
pub fn reference_matches(text: &str, reference: &str) -> bool {
    text.split_whitespace().eq(reference.split_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn explicit_german_replay_preserves_reference_and_repetition_options()
    -> Result<(), Box<dyn std::error::Error>> {
        let arguments: Vec<OsString> = [
            "model.bin",
            "dictation.wav",
            "--language",
            "de",
            "--repeat",
            "3",
            "--reference",
            "reference.txt",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        let options = Options::parse(&arguments)?;
        assert_eq!(
            options.language,
            everlast_dictation::transcription::Language::German
        );
        assert_eq!(options.repeats, 3);
        assert_eq!(options.reference, Some("reference.txt".into()));
        assert!(!options.legacy);
        Ok(())
    }

    fn arguments(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn legacy_jfk_invocation_retains_english_two_runs_and_known_phrase_check()
    -> Result<(), Box<dyn std::error::Error>> {
        let options = Options::parse(&arguments(&["model.bin", "jfk.wav"]))?;
        assert!(options.legacy);
        assert_eq!(options.language, Language::English);
        assert_eq!(options.repeats, 2);
        assert!(options.reference.is_none());
        Ok(())
    }

    #[test]
    fn malformed_replay_arguments_are_rejected_without_falling_back_to_defaults() {
        for values in [
            vec![],
            vec!["model.bin"],
            vec!["model.bin", "input.wav", "--language"],
            vec!["model.bin", "input.wav", "--repeat", "3"],
            vec![
                "model.bin",
                "input.wav",
                "--language",
                "de",
                "--language",
                "en",
            ],
            vec!["model.bin", "input.wav", "--language", "de", "--other", "x"],
            vec![
                "model.bin",
                "input.wav",
                "--language",
                "de",
                "--reference",
                "",
            ],
            vec!["", "input.wav"],
            vec![
                "model.bin",
                "input.wav",
                "--language",
                "de",
                "--repeat",
                "1",
                "--reference",
                "r",
                "--extra",
                "x",
            ],
        ] {
            assert!(matches!(
                Options::parse(&arguments(&values)),
                Err(InputError::Arguments)
            ));
        }
        for language in ["fr", "DE", "auto", ""] {
            assert!(matches!(
                Options::parse(&arguments(&["model", "audio", "--language", language])),
                Err(InputError::Language)
            ));
        }
        for count in ["0", "31", "-1", "1.5", "invalid", "4294967296"] {
            assert!(matches!(
                Options::parse(&arguments(&[
                    "model",
                    "audio",
                    "--language",
                    "en",
                    "--repeat",
                    count
                ])),
                Err(InputError::Repeats)
            ));
        }
    }

    #[test]
    fn repetition_limit_is_inclusive() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            Options::parse(&arguments(&[
                "m",
                "a",
                "--language",
                "en",
                "--repeat",
                "30"
            ]))?
            .repeats,
            30
        );
        Ok(())
    }

    #[test]
    fn references_are_nonempty_bounded_utf8_and_comparison_preserves_words_and_punctuation()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let reference = directory.path().join("reference.txt");
        fs::write(&reference, "Guten Tag.\nNoch ein Satz.")?;
        let expected = load_reference(&reference)?;
        assert!(reference_matches("Guten  Tag. Noch ein Satz.", &expected));
        assert!(!reference_matches("guten Tag. Noch ein Satz.", &expected));
        assert!(!reference_matches("Guten Tag Noch ein Satz.", &expected));
        fs::write(&reference, " \n\t")?;
        assert_eq!(load_reference(&reference), Err(InputError::ReferenceEmpty));
        fs::write(&reference, [0xff])?;
        assert_eq!(load_reference(&reference), Err(InputError::ReferenceUtf8));
        fs::write(&reference, "ä".repeat(MAX_REFERENCE_BYTES / 2))?;
        assert_eq!(load_reference(&reference)?.len(), MAX_REFERENCE_BYTES);
        fs::write(&reference, "x".repeat(MAX_REFERENCE_BYTES + 1))?;
        assert_eq!(load_reference(&reference), Err(InputError::ReferenceLength));
        assert_eq!(
            load_reference(directory.path()),
            Err(InputError::ReferenceIo)
        );
        Ok(())
    }

    #[test]
    fn valid_pcm_extrema_are_loaded_and_other_wav_formats_or_corruption_are_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let wav = directory.path().join("fixture.wav");
        let valid = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&wav, valid)?;
        for sample in [i16::MIN, 0, i16::MAX] {
            writer.write_sample(sample)?;
        }
        writer.finalize()?;
        assert_eq!(load_audio(&wav)?, [-1.0, 0.0, 0.999_969_5]);
        for spec in [
            hound::WavSpec {
                sample_rate: 8000,
                ..valid
            },
            hound::WavSpec {
                channels: 2,
                ..valid
            },
        ] {
            let mut writer = hound::WavWriter::create(&wav, spec)?;
            writer.write_sample(0i16)?;
            writer.write_sample(0i16)?;
            writer.finalize()?;
            assert_eq!(load_audio(&wav), Err(InputError::FixtureFormat));
        }
        let writer = hound::WavWriter::create(&wav, valid)?;
        writer.finalize()?;
        assert_eq!(load_audio(&wav), Err(InputError::FixtureLength));
        fs::write(&wav, b"not a WAV")?;
        assert_eq!(load_audio(&wav), Err(InputError::FixtureFormat));
        assert_eq!(load_audio(directory.path()), Err(InputError::FixtureIo));
        Ok(())
    }

    #[test]
    fn oversized_declared_duration_and_file_size_are_rejected_before_sample_allocation()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let wav = directory.path().join("long.wav");
        // A syntactically complete PCM header declares 300 s + one sample.
        // No large sample fixture is allocated or decoded for this limit test.
        let data_bytes = 9_600_002u32;
        let mut header = Vec::new();
        header.extend_from_slice(b"RIFF");
        header.extend_from_slice(&(36 + data_bytes).to_le_bytes());
        header.extend_from_slice(b"WAVEfmt ");
        header.extend_from_slice(&16u32.to_le_bytes());
        header.extend_from_slice(&1u16.to_le_bytes());
        header.extend_from_slice(&1u16.to_le_bytes());
        header.extend_from_slice(&16000u32.to_le_bytes());
        header.extend_from_slice(&32000u32.to_le_bytes());
        header.extend_from_slice(&2u16.to_le_bytes());
        header.extend_from_slice(&16u16.to_le_bytes());
        header.extend_from_slice(b"data");
        header.extend_from_slice(&data_bytes.to_le_bytes());
        fs::write(&wav, header)?;
        assert_eq!(load_audio(&wav), Err(InputError::FixtureLength));
        File::options()
            .write(true)
            .open(&wav)?
            .set_len(MAX_WAV_BYTES + 1)?;
        assert_eq!(load_audio(&wav), Err(InputError::FixtureLength));
        Ok(())
    }
}
