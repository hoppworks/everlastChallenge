#![cfg(feature = "mlx")]

use mlxrs::{
    Array,
    memory::{CacheLimitGuard, cache_memory, set_cache_limit},
};
use std::sync::Mutex;

static TEST_CACHE: Mutex<()> = Mutex::new(());

#[cfg(feature = "mlx-encoder-f16")]
#[test]
fn f16_runtime_loader_requires_the_exclusive_cache_owner() -> Result<(), Box<dyn std::error::Error>>
{
    let _serial = TEST_CACHE.lock().map_err(|_| "test cache lock poisoned")?;
    let policy = CacheLimitGuard::install(16 * 1024 * 1024)?;
    let directory = tempfile::tempdir()?;
    let error = everlast_dictation::transcription::mlx::MlxEngine::load_reference_encoder(
        &directory.path().join("weights.safetensors"),
    )
    .err()
    .ok_or("loader unexpectedly succeeded")?;
    assert!(
        matches!(
            error,
            everlast_dictation::transcription::InferenceError::Context
        ),
        "runtime loader must acquire the policy before loading: {error}"
    );
    policy.finish()?;
    Ok(())
}

#[test]
fn cache_policy_has_one_owner_and_restores_the_original_limit()
-> Result<(), Box<dyn std::error::Error>> {
    let _serial = TEST_CACHE.lock().map_err(|_| "test cache lock poisoned")?;
    let sentinel = 384 * 1024 * 1024;
    let original = set_cache_limit(sentinel)?;
    let policy = CacheLimitGuard::install(16 * 1024 * 1024)?;
    assert_eq!(policy.old_limit(), sentinel);
    assert_eq!(policy.limit(), 16 * 1024 * 1024);
    assert!(CacheLimitGuard::install(8 * 1024 * 1024).is_err());
    assert!(set_cache_limit(8 * 1024 * 1024).is_err());
    policy.finish()?;
    assert_eq!(set_cache_limit(original)?, sentinel);
    Ok(())
}

#[test]
fn finished_policy_releases_unused_buffers() -> Result<(), Box<dyn std::error::Error>> {
    let _serial = TEST_CACHE.lock().map_err(|_| "test cache lock poisoned")?;
    let sentinel = 384 * 1024 * 1024;
    let original = set_cache_limit(sentinel)?;
    let policy = CacheLimitGuard::install(16 * 1024 * 1024)?;
    let mut buffer = Array::from_slice(&vec![1.0f32; 1024 * 1024], &[1024 * 1024])?;
    buffer.eval()?;
    drop(buffer);
    assert!(
        cache_memory()? >= 4 * 1024 * 1024,
        "fixture must leave reusable buffers"
    );
    policy.finish()?;
    assert!(
        cache_memory()? <= 1024 * 1024,
        "finished owner retained its unused buffers"
    );
    assert_eq!(set_cache_limit(original)?, sentinel);
    Ok(())
}

#[cfg(feature = "measurement")]
#[test]
fn failed_model_load_restores_the_cache_policy() -> Result<(), Box<dyn std::error::Error>> {
    let _serial = TEST_CACHE.lock().map_err(|_| "test cache lock poisoned")?;
    let sentinel = 384 * 1024 * 1024;
    let original = set_cache_limit(sentinel)?;
    let directory = tempfile::tempdir()?;
    let result = everlast_dictation::transcription::mlx::MlxEngine::
        load_reference_encoder_with_cache_budget_for_measurement(
            &directory.path().join("weights.safetensors"),
            8 * 1024 * 1024,
        );
    assert!(result.is_err());
    assert_eq!(set_cache_limit(original)?, sentinel);
    Ok(())
}

#[test]
fn unwind_frees_unused_buffers_and_allows_a_new_owner() -> Result<(), Box<dyn std::error::Error>> {
    let _serial = TEST_CACHE.lock().map_err(|_| "test cache lock poisoned")?;
    let sentinel = 384 * 1024 * 1024;
    let original = set_cache_limit(sentinel)?;
    let unwound = std::panic::catch_unwind(|| -> Result<(), Box<dyn std::error::Error>> {
        let _policy = CacheLimitGuard::install(16 * 1024 * 1024)?;
        let mut buffer = Array::from_slice(&vec![1.0f32; 1024 * 1024], &[1024 * 1024])?;
        buffer.eval()?;
        drop(buffer);
        // Start an intentional test unwind without a production panic shortcut.
        std::panic::resume_unwind(Box::new("simulated worker unwind"));
    });
    assert!(unwound.is_err());
    assert_eq!(set_cache_limit(sentinel)?, sentinel);
    assert!(cache_memory()? <= 1024 * 1024);
    CacheLimitGuard::install(8 * 1024 * 1024)?.finish()?;
    assert_eq!(set_cache_limit(original)?, sentinel);
    Ok(())
}
