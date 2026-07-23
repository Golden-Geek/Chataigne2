use std::{path::PathBuf, sync::Arc};

use crate::SampleRate;

use super::super::{AssetCache, AudioSourceFingerprint, ResidentAssetKey, ResidentAudioAsset};

#[test]
fn cache_rejects_invalid_limits() {
    assert!(AssetCache::new(2, 1).is_err());
}

#[test]
fn cache_hits_invalidates_changed_files_and_evicts_only_off_realtime() {
    let mut cache = AssetCache::new(64, 64).unwrap();
    let first = asset("a.wav", 1, 8);
    let first_key = first.key().clone();
    assert!(cache.insert(Arc::clone(&first)).cached);
    assert!(Arc::ptr_eq(&cache.get(&first_key).unwrap(), &first));

    let changed = asset("a.wav", 2, 8);
    assert!(cache.insert(Arc::clone(&changed)).cached);
    assert!(cache.get(&first_key).is_none());
    assert_eq!(first.frames(), 8, "active immutable generation remains usable");

    let other = asset("b.wav", 1, 12);
    let result = cache.insert(Arc::clone(&other));
    assert!(result.cached);
    assert_eq!(result.evicted_entries, 1);
    let observation = cache.observation();
    assert_eq!(observation.entries, 1);
    assert_eq!(observation.resident_bytes, other.memory_bytes());
    assert!(observation.hits >= 1);
    assert!(observation.misses >= 1);
    assert!(observation.invalidations >= 1);
    assert!(observation.evictions >= 1);
}

fn asset(path: &str, source_length: u64, frames: usize) -> Arc<ResidentAudioAsset> {
    Arc::new(
        ResidentAudioAsset::new(
            ResidentAssetKey {
                source: AudioSourceFingerprint {
                    canonical_path: PathBuf::from(path),
                    length_bytes: source_length,
                    modified_nanos: u128::from(source_length),
                },
                track: 0,
                engine_sample_rate: SampleRate::default(),
            },
            1,
            frames,
            vec![0.25; frames],
        )
        .unwrap(),
    )
}
