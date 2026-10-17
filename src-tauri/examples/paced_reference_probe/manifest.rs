//! Exactly DE/EN by 5/10/15 seconds, selected without reading model results.

use crate::input;
use serde::Deserialize;
use std::collections::BTreeSet;
pub struct Case {
    pub id: String,
    pub language: String,
    pub seconds: u32,
    pub reference: String,
    pub audio: input::Audio,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    unicode_version: String,
    cases: Vec<Fields>,
}
#[derive(Deserialize)]
struct Fields {
    class: String,
    split: String,
    reference: String,
}
pub fn parse(bytes: &[u8]) -> Result<Vec<Case>, String> {
    let declared = input::parse_manifest(bytes).map_err(|e| e.to_string())?;
    let metadata: Metadata = serde_json::from_slice(bytes).map_err(
        |_| "central manifest requires explicit class, split, reference and Unicode version",
    )?;
    if declared.cases.len() != 6 || metadata.unicode_version != "13.0.0" {
        return Err("central manifest must contain exactly six cases under Unicode13.0.0".into());
    }
    let mut slots = BTreeSet::new();
    let mut hashes = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut cases = Vec::new();
    for (case, fields) in declared.cases.into_iter().zip(metadata.cases) {
        let seconds = case.audio.frames / 16000;
        if fields.class != "clean"
            || fields.split != "acceptance"
            || fields.reference.trim().is_empty()
            || fields.reference.len() > 256 * 1024
            || ![5, 10, 15].contains(&seconds)
            || case.audio.frames != seconds * 16000
            || !slots.insert((case.language.clone(), seconds))
            || !hashes.insert(case.audio.sha256.clone())
            || !paths.insert(case.audio.path.clone())
        {
            return Err("central cases require unique DE/EN x5/10/15s clean acceptance slots, audio assets and nonempty bounded references".into());
        }
        cases.push(Case {
            id: case.id,
            language: case.language,
            seconds,
            reference: fields.reference,
            audio: case.audio,
        });
    }
    cases.sort_by(|a, b| (&a.language, a.seconds).cmp(&(&b.language, b.seconds)));
    Ok(cases)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    pub(super) fn document() -> serde_json::Value {
        json!({"schemaVersion":1,"unicodeVersion":"13.0.0","cases":
            (["en","de"].into_iter().flat_map(|language| [15,5,10].map(move |seconds|json!({
                "id":format!("{language}-{seconds}"),"language":language,"class":"clean","split":"acceptance",
                "reference":"published independent reference","audio":{"path":format!("audio/{language}-{seconds}.wav"),
                "sha256":format!("{:064x}",seconds+if language=="de"{0}else{100}),"frames":seconds*16000,"sampleRate":16000}
            }))).collect::<Vec<_>>())})
    }
    #[test]
    fn six_slots_are_validated_and_ordered_by_language_then_exact_duration()
    -> Result<(), Box<dyn std::error::Error>> {
        let cases = parse(&serde_json::to_vec(&document())?)?;
        assert_eq!(
            cases
                .iter()
                .map(|c| (c.language.as_str(), c.seconds))
                .collect::<Vec<_>>(),
            [
                ("de", 5),
                ("de", 10),
                ("de", 15),
                ("en", 5),
                ("en", 10),
                ("en", 15)
            ]
        );
        for (field, value) in [
            ("class", json!("challenging")),
            ("split", json!("calibration")),
            ("reference", json!(" ")),
        ] {
            let mut wrong = document();
            wrong["cases"][0][field] = value;
            assert!(parse(&serde_json::to_vec(&wrong)?).is_err());
        }
        for (field, value) in [
            ("frames", json!(239999)),
            ("sampleRate", json!(48000)),
            ("path", json!("../escape.wav")),
            ("sha256", json!("invalid")),
        ] {
            let mut wrong = document();
            wrong["cases"][0]["audio"][field] = value;
            assert!(parse(&serde_json::to_vec(&wrong)?).is_err());
        }
        Ok(())
    }
    #[test]
    fn missing_extra_duplicate_slots_and_reused_audio_are_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let original = document();
        for size in [5, 7] {
            let mut wrong = original.clone();
            let cases = wrong["cases"].as_array_mut().ok_or("cases")?;
            if size == 5 {
                cases.pop();
            } else {
                cases.push(cases[0].clone());
            }
            assert!(parse(&serde_json::to_vec(&wrong)?).is_err());
        }
        for target in ["id", "slot", "hash"] {
            let mut wrong = original.clone();
            match target {
                "id" => wrong["cases"][1]["id"] = wrong["cases"][0]["id"].clone(),
                "hash" => {
                    wrong["cases"][1]["audio"]["sha256"] =
                        wrong["cases"][0]["audio"]["sha256"].clone()
                }
                _ => {
                    wrong["cases"][1]["language"] = wrong["cases"][0]["language"].clone();
                    wrong["cases"][1]["audio"]["frames"] =
                        wrong["cases"][0]["audio"]["frames"].clone();
                }
            }
            assert!(parse(&serde_json::to_vec(&wrong)?).is_err());
        }
        Ok(())
    }
}
