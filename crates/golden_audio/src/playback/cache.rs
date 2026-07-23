use std::{collections::HashMap, sync::Arc};

use crate::{AudioError, assert_not_realtime};

use super::{ResidentAssetKey, ResidentAudioAsset};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CacheObservation {
    pub entries: usize,
    pub resident_bytes: u64,
    pub hits: u64,
    pub misses: u64,
    pub invalidations: u64,
    pub evictions: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheInsertResult {
    pub cached: bool,
    pub evicted_entries: usize,
}

#[derive(Debug)]
struct CacheEntry {
    asset: Arc<ResidentAudioAsset>,
    last_access: u64,
}

#[derive(Debug)]
pub struct AssetCache {
    entries: HashMap<ResidentAssetKey, CacheEntry>,
    resident_threshold_bytes: u64,
    budget_bytes: u64,
    resident_bytes: u64,
    access_sequence: u64,
    hits: u64,
    misses: u64,
    invalidations: u64,
    evictions: u64,
}

impl AssetCache {
    pub fn new(resident_threshold_bytes: u64, budget_bytes: u64) -> Result<Self, AudioError> {
        if resident_threshold_bytes == 0 || budget_bytes == 0 || resident_threshold_bytes > budget_bytes {
            return Err(AudioError::invalid_configuration(
                "asset cache requires a positive resident threshold not exceeding its budget",
            ));
        }
        Ok(Self {
            entries: HashMap::new(),
            resident_threshold_bytes,
            budget_bytes,
            resident_bytes: 0,
            access_sequence: 0,
            hits: 0,
            misses: 0,
            invalidations: 0,
            evictions: 0,
        })
    }

    pub fn get(&mut self, key: &ResidentAssetKey) -> Option<Arc<ResidentAudioAsset>> {
        assert_not_realtime("resident asset cache lookup");
        self.access_sequence = self.access_sequence.saturating_add(1);
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_access = self.access_sequence;
            self.hits = self.hits.saturating_add(1);
            return Some(Arc::clone(&entry.asset));
        }
        self.misses = self.misses.saturating_add(1);
        None
    }

    pub fn insert(&mut self, asset: Arc<ResidentAudioAsset>) -> CacheInsertResult {
        assert_not_realtime("resident asset cache insertion and eviction");
        let bytes = asset.memory_bytes();
        if bytes > self.resident_threshold_bytes || bytes > self.budget_bytes {
            return CacheInsertResult {
                cached: false,
                evicted_entries: 0,
            };
        }
        let key = asset.key().clone();
        self.invalidate_older_source_generations(&key);
        if self.entries.contains_key(&key) {
            return CacheInsertResult {
                cached: true,
                evicted_entries: 0,
            };
        }
        self.access_sequence = self.access_sequence.saturating_add(1);
        let mut evicted_entries = 0;
        while self.resident_bytes.saturating_add(bytes) > self.budget_bytes {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_access)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            self.remove(&oldest);
            self.evictions = self.evictions.saturating_add(1);
            evicted_entries += 1;
        }
        self.resident_bytes = self.resident_bytes.saturating_add(bytes);
        self.entries.insert(
            key,
            CacheEntry {
                asset,
                last_access: self.access_sequence,
            },
        );
        CacheInsertResult {
            cached: true,
            evicted_entries,
        }
    }

    pub fn invalidate_path(&mut self, path: &std::path::Path) -> usize {
        assert_not_realtime("resident asset cache invalidation");
        let keys: Vec<_> = self
            .entries
            .keys()
            .filter(|key| key.source.canonical_path == path)
            .cloned()
            .collect();
        for key in &keys {
            self.remove(key);
        }
        self.invalidations = self
            .invalidations
            .saturating_add(u64::try_from(keys.len()).unwrap_or(u64::MAX));
        keys.len()
    }

    #[must_use]
    pub fn observation(&self) -> CacheObservation {
        CacheObservation {
            entries: self.entries.len(),
            resident_bytes: self.resident_bytes,
            hits: self.hits,
            misses: self.misses,
            invalidations: self.invalidations,
            evictions: self.evictions,
        }
    }

    fn invalidate_older_source_generations(&mut self, new_key: &ResidentAssetKey) {
        let stale: Vec<_> = self
            .entries
            .keys()
            .filter(|key| {
                key.source.canonical_path == new_key.source.canonical_path
                    && (key.source != new_key.source || key.engine_sample_rate != new_key.engine_sample_rate)
            })
            .cloned()
            .collect();
        for key in &stale {
            self.remove(key);
        }
        self.invalidations = self
            .invalidations
            .saturating_add(u64::try_from(stale.len()).unwrap_or(u64::MAX));
    }

    fn remove(&mut self, key: &ResidentAssetKey) {
        if let Some(entry) = self.entries.remove(key) {
            self.resident_bytes = self.resident_bytes.saturating_sub(entry.asset.memory_bytes());
        }
    }
}
