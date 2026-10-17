//! Only resolves the active layout; does not select layouts or post input.
use macos_integration::keyboard::{PasteKey, modifiers_held};
use objc2::MainThreadMarker;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let main = MainThreadMarker::new().ok_or("not on main thread")?;
    let key = PasteKey::resolve(main)?;
    println!(
        "Current-layout Command+V resolved to physical key {}",
        key.key_code()
    );
    if !key.is_current(main)? {
        return Err("layout changed while probing".into());
    }
    println!(
        "Layout recheck passed; physical modifier held: {}",
        modifiers_held()
    );
    Ok(())
}
