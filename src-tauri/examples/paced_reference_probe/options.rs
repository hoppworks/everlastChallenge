//! No implicit model, run count, language, or mutable parameter default.
use everlast_dictation::model::{ModelDefinition, REFERENCE, SMALL};
use std::{ffi::OsString, path::PathBuf};
pub struct Options {
    pub model: PathBuf,
    pub definition: ModelDefinition,
    pub manifest: PathBuf,
    pub parameters: PathBuf,
    pub output: PathBuf,
}
impl Options {
    pub fn parse(args: &[OsString]) -> Result<Self, String> {
        if args.len() != 5 {
            return Err("usage: paced_reference_probe MODEL_PATH reference|small SIX_CASE_MANIFEST PARAMETERS_JSON NEW_OUTPUT_JSON".into());
        }
        if args
            .iter()
            .any(|v| !(1..=4096).contains(&v.as_encoded_bytes().len()))
        {
            return Err("each argument must contain1..4096 bytes".into());
        }
        let definition = match args[1].to_str() {
            Some("reference") => REFERENCE,
            Some("small") => SMALL,
            _ => return Err("model must be reference or small".into()),
        };
        Ok(Self {
            model: args[0].clone().into(),
            definition,
            manifest: args[2].clone().into(),
            parameters: args[3].clone().into(),
            output: args[4].clone().into(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cli_requires_all_explicit_paths_and_model_without_run_count_override() -> Result<(), String>
    {
        let args: [OsString; 5] = [
            "model",
            "reference",
            "six.json",
            "parameters.json",
            "new.json",
        ]
        .map(Into::into);
        let parsed = Options::parse(&args)?;
        assert_eq!(parsed.definition.id(), REFERENCE.id());
        assert_eq!(parsed.manifest, PathBuf::from("six.json"));
        assert_eq!(parsed.parameters, PathBuf::from("parameters.json"));
        assert_eq!(parsed.output, PathBuf::from("new.json"));
        assert_eq!(parsed.model, PathBuf::from("model"));
        for n in 0..5 {
            assert!(Options::parse(&args[..n]).is_err());
        }
        let mut extra = args.to_vec();
        extra.push("30".into());
        assert!(Options::parse(&extra).is_err());
        for (index, value) in [
            (0, ""),
            (1, "auto"),
            (1, "REFERENCE"),
            (2, ""),
            (3, ""),
            (4, ""),
        ] {
            let mut wrong = args.clone();
            wrong[index] = value.into();
            assert!(Options::parse(&wrong).is_err());
        }
        let mut wrong = args.clone();
        wrong[0] = "x".repeat(4097).into();
        assert!(Options::parse(&wrong).is_err());
        let mut small = args;
        small[1] = "small".into();
        assert_eq!(Options::parse(&small)?.definition.id(), SMALL.id());
        Ok(())
    }
}
