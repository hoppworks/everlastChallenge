//! Explicit bounded developer CLI; no legacy/default-language invocation.

use everlast_dictation::{
    model::{ModelDefinition, REFERENCE, SMALL},
    transcription::Language,
};
use std::{ffi::OsString, path::PathBuf};

#[derive(Debug)]
pub struct Options {
    pub model: PathBuf,
    pub definition: ModelDefinition,
    pub wav: PathBuf,
    pub language: Language,
    pub parameters: PathBuf,
    pub output: PathBuf,
    pub runs: u32,
    pub reference: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionsError {
    ArgumentCount,
    ArgumentLength,
    Model,
    Language,
    Runs,
}
impl std::fmt::Display for OptionsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ArgumentCount => "usage: paced_runtime_probe MODEL_PATH reference|small WAV de|en PARAMETERS_JSON OUTPUT_JSON RUNS [REFERENCE_UTF8]",
            Self::ArgumentLength => "Each argument must contain 1 to 4096 bytes",
            Self::Model => "Model selection must be reference or small",
            Self::Language => "Language must be de or en",
            Self::Runs => "RUNS must be an integer from 1 to 30",
        })
    }
}
impl std::error::Error for OptionsError {}

impl Options {
    pub fn parse(arguments: &[OsString]) -> Result<Self, OptionsError> {
        if !(7..=8).contains(&arguments.len()) {
            return Err(OptionsError::ArgumentCount);
        }
        if arguments
            .iter()
            .any(|argument| !(1..=4096).contains(&argument.as_encoded_bytes().len()))
        {
            return Err(OptionsError::ArgumentLength);
        }
        let definition = match arguments[1].to_str() {
            Some("reference") => REFERENCE,
            Some("small") => SMALL,
            _ => return Err(OptionsError::Model),
        };
        let language = match arguments[3].to_str() {
            Some("de") => Language::German,
            Some("en") => Language::English,
            _ => return Err(OptionsError::Language),
        };
        let runs = arguments[6]
            .to_str()
            .filter(|text| text.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|text| text.parse::<u32>().ok())
            .filter(|runs| (1..=30).contains(runs))
            .ok_or(OptionsError::Runs)?;
        Ok(Self {
            model: PathBuf::from(&arguments[0]),
            definition,
            wav: PathBuf::from(&arguments[2]),
            language,
            parameters: PathBuf::from(&arguments[4]),
            output: PathBuf::from(&arguments[5]),
            runs,
            reference: arguments.get(7).map(PathBuf::from),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn arguments() -> Vec<OsString> {
        [
            "model.bin",
            "reference",
            "audio.wav",
            "de",
            "parameters.json",
            "report.json",
            "30",
        ]
        .into_iter()
        .map(OsString::from)
        .collect()
    }

    #[test]
    fn explicit_options_preserve_paths_model_language_and_run_count()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut arguments = arguments();
        let options = Options::parse(&arguments)?;
        assert_eq!(options.model, PathBuf::from("model.bin"));
        assert_eq!(options.definition.id(), REFERENCE.id());
        assert_eq!(options.wav, PathBuf::from("audio.wav"));
        assert_eq!(options.language, Language::German);
        assert_eq!(options.parameters, PathBuf::from("parameters.json"));
        assert_eq!(options.output, PathBuf::from("report.json"));
        assert_eq!(options.runs, 30);
        assert_eq!(options.reference, None);
        arguments[1] = "small".into();
        arguments[3] = "en".into();
        arguments[6] = "1".into();
        arguments.push("reference.txt".into());
        let options = Options::parse(&arguments)?;
        assert_eq!(options.definition.id(), SMALL.id());
        assert_eq!(options.language, Language::English);
        assert_eq!(options.runs, 1);
        assert_eq!(options.reference, Some(PathBuf::from("reference.txt")));
        Ok(())
    }
    #[test]
    fn no_legacy_or_extra_arguments_and_no_implicit_model_language_or_runs() {
        for count in 0..7 {
            assert!(matches!(
                Options::parse(&arguments()[..count]),
                Err(OptionsError::ArgumentCount)
            ));
        }
        let mut extra = arguments();
        extra.extend([OsString::from("reference.txt"), OsString::from("extra")]);
        assert!(matches!(
            Options::parse(&extra),
            Err(OptionsError::ArgumentCount)
        ));
        for (index, value, expected) in [
            (1, "large", OptionsError::Model),
            (1, "REFERENCE", OptionsError::Model),
            (3, "auto", OptionsError::Language),
            (3, "DE", OptionsError::Language),
            (6, "0", OptionsError::Runs),
            (6, "31", OptionsError::Runs),
            (6, "-1", OptionsError::Runs),
            (6, "+1", OptionsError::Runs),
            (6, "1.5", OptionsError::Runs),
            (6, " 1", OptionsError::Runs),
            (6, "4294967296", OptionsError::Runs),
        ] {
            let mut values = arguments();
            values[index] = value.into();
            assert!(matches!(Options::parse(&values), Err(error) if error == expected));
        }
    }

    #[test]
    fn every_argument_including_optional_reference_has_a_byte_bound() {
        let mut values = arguments();
        values.push("reference.txt".into());
        for index in 0..values.len() {
            for invalid in [
                OsString::new(),
                OsString::from("a".repeat(4097)),
                OsString::from("ä".repeat(2049)),
            ] {
                let mut invalid_values = values.clone();
                invalid_values[index] = invalid;
                assert!(matches!(
                    Options::parse(&invalid_values),
                    Err(OptionsError::ArgumentLength)
                ));
            }
        }
        for index in [0, 2, 4, 5, 7] {
            let mut boundary = values.clone();
            boundary[index] = "ä".repeat(2048).into();
            assert!(Options::parse(&boundary).is_ok());
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_are_preserved_but_non_utf8_selectors_are_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::ffi::OsStringExt;
        let non_utf8 = OsString::from_vec(vec![b'a', 255]);
        let mut values = arguments();
        values[0] = non_utf8.clone();
        assert_eq!(
            Options::parse(&values)?.model.as_os_str(),
            non_utf8.as_os_str()
        );
        for (index, expected) in [
            (1, OptionsError::Model),
            (3, OptionsError::Language),
            (6, OptionsError::Runs),
        ] {
            let mut invalid = arguments();
            invalid[index] = non_utf8.clone();
            assert!(matches!(Options::parse(&invalid), Err(error) if error == expected));
        }
        Ok(())
    }
}
