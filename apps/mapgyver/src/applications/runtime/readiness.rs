//! Native evidence is sampled only on the existing application IO worker, never the actor.

use super::*;

#[derive(Default)]
pub(super) struct Observer {
    #[cfg(all(windows, feature = "native-video"))]
    native: Option<(uuid::Uuid, golden_video::native::SpoutProbe)>,
    next: Option<Instant>,
}
impl Observer {
    pub fn release_completed(&mut self) {
        #[cfg(all(windows, feature = "native-video"))]
        if self
            .native
            .as_mut()
            .is_some_and(|(_, probe)| !matches!(probe.is_idle(), Ok(true)))
        {
            return;
        }
        *self = Self::default();
    }

    pub fn sample(&mut self, launch: uuid::Uuid, rule: &ApplicationReadiness) -> Option<(String, bool)> {
        let ApplicationReadiness::Spout {
            sender, first_frame, ..
        } = rule
        else {
            return None;
        };
        let now = Instant::now();
        if self.next.is_some_and(|next| now < next) {
            return None;
        }
        self.next = Some(now + std::time::Duration::from_millis(100));
        self.native_sample(launch, sender, *first_frame)
    }

    #[cfg(all(windows, feature = "native-video"))]
    fn native_sample(&mut self, launch: uuid::Uuid, sender: &str, first_frame: bool) -> Option<(String, bool)> {
        use golden_video::native::{SpoutProbe, adapters};
        // A launch changes only after the preceding owned process has retired. Its probe is
        // drained on this same worker before a new observer can acquire bounded storage.
        if self.native.as_ref().is_some_and(|(current, _)| *current != launch) {
            self.native.take();
        }
        if self.native.is_none() {
            static BUDGET: OnceLock<Result<golden_io::PayloadPool, String>> = OnceLock::new();
            let budget = BUDGET
                .get_or_init(|| {
                    golden_io::PayloadPool::new("mapgyver-readiness-images", MAX_APPLICATIONS, 256 * 1024 * 1024)
                        .map_err(|error| error.to_string())
                })
                .as_ref()
                .ok()?
                .clone();
            let adapter = adapters().ok()?.into_iter().find(|adapter| !adapter.software)?.id;
            self.native = Some((launch, SpoutProbe::new(sender, adapter, budget).ok()?));
        }
        let snapshot = self.native.as_mut()?.1.poll(first_frame).ok()?;
        (snapshot.present && (!first_frame || snapshot.first_frame)).then(|| (sender.to_owned(), snapshot.first_frame))
    }

    #[cfg(not(all(windows, feature = "native-video")))]
    fn native_sample(&mut self, _: uuid::Uuid, _: &str, _: bool) -> Option<(String, bool)> {
        None
    }
}
