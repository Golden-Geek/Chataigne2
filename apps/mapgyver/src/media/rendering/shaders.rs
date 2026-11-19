//! Compile only demanded program revisions, on one bounded worker per renderer.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::{Arc, OnceLock};

use golden_io::{BoundedWorker, RetirementPool, WorkerLimits, WorkerNotifier};
use golden_video::shader::{CompileError, FragmentProgram};

use crate::media::capture::CapturedChannels;
use crate::media::protocol::{ChannelReference, ChannelSource};

type Compiled = Result<FragmentProgram, CompileError>;
type Outcome = (Arc<str>, Compiled);

pub(super) struct Shaders {
    worker: BoundedWorker<Arc<str>, Outcome>,
    entries: HashMap<Arc<str>, Option<Compiled>>,
    running: bool,
}

impl Shaders {
    pub(super) fn new(notify: WorkerNotifier) -> Result<Self, String> {
        static WORKERS: OnceLock<RetirementPool> = OnceLock::new();
        let pool = WORKERS.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()));
        let worker = BoundedWorker::<Arc<str>, Outcome>::spawn_notified(
            "mapgyver-shader-compile",
            pool,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 1,
                event_weight: 1,
            },
            notify,
            |worker| {
                while let Some(code) = worker.receive() {
                    let program = FragmentProgram::compile_glsl(&code);
                    if worker.publish_weighted((code, program), 1).is_err() {
                        break;
                    }
                }
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            worker,
            entries: HashMap::new(),
            running: false,
        })
    }

    pub(super) fn sync(&mut self, capture: &CapturedChannels, references: &[ChannelReference]) {
        let references = references.iter().copied().collect::<HashSet<_>>();
        let wanted = capture
            .channels
            .iter()
            .filter(|channel| channel.enabled && references.contains(&channel.reference))
            .filter_map(|channel| match &channel.definition.as_ref()?.source {
                ChannelSource::Shader { code } if !code.trim().is_empty() => Some(code.clone()),
                _ => None,
            })
            .collect::<HashSet<_>>();
        self.entries.retain(|code, _| wanted.contains(code));
        for code in wanted {
            self.entries.entry(code).or_insert(None);
        }
        self.service();
    }

    pub(super) fn service(&mut self) -> bool {
        let mut outcomes = Vec::new();
        self.worker.drain_events(&mut outcomes, NonZeroUsize::MIN);
        let mut changed = false;
        for (code, outcome) in outcomes {
            self.running = false;
            if let Some(entry) = self.entries.get_mut(&code) {
                if let Err(error) = &outcome {
                    eprintln!("[mapgyver-shader] {error}");
                }
                *entry = Some(outcome);
                changed = true;
            }
        }
        if !self.running
            && let Some((code, _)) = self.entries.iter().find(|(_, entry)| entry.is_none())
        {
            self.running = self.worker.send_weighted(code.clone(), 1).is_ok();
        }
        changed
    }

    pub(super) fn get(&self, code: &str) -> Option<&Compiled> {
        self.entries.get(code)?.as_ref()
    }
}
