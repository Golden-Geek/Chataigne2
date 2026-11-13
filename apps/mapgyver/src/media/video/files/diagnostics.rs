//! First-install source observations; no periodic logger or engine-thread inspection.
use super::*;

impl Run {
    pub(super) fn report_cpu_layouts(&self, reader: &Reader, lease: &Lease, retained_cpu_image_bytes: Option<u64>) {
        if !std::env::var_os("MAPGYVER_NATIVE_DIAGNOSTICS").is_some_and(|value| !value.is_empty()) {
            return;
        }
        eprintln!(
            "[mapgyver-video-cpu] {}",
            serde_json::json!({
                "owner": self.owner,
                "reference": self.reference,
                "file": self.file,
                "reservation_bytes": lease.bytes,
                "cpu_admission": self.budget.snapshot(),
                "reader": reader.cpu_layouts(),
                "retained_cpu_image_bytes": retained_cpu_image_bytes,
            })
        );
    }
}
