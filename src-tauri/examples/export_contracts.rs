use everlast_dictation::contracts::Snapshot;
use ts_rs::TS;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or("output directory required")?;
    Snapshot::export_all_to(output)?;
    Ok(())
}
