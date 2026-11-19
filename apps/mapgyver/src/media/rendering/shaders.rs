//! Compile only demanded program revisions, on one bounded worker per renderer.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use golden_io::{BoundedWorker, RetirementPool, WorkerLimits, WorkerNotifier};
use golden_video::shader::{CompileError, FragmentProgram};

use crate::media::capture::CapturedChannels;
use crate::media::protocol::{ChannelReference, ChannelSource, ShaderInput};
use crate::media::resolution::ChannelAvailability;

mod files;

type Compiled = Result<FragmentProgram, CompileError>;
type Outcome = (Arc<str>, Compiled);

pub(super) struct Shaders {
    worker: BoundedWorker<Arc<str>, Outcome>,
    entries: HashMap<Arc<str>, Option<Compiled>>,
    running: bool,
    root: PathBuf,
    notify: WorkerNotifier,
    files: Option<files::Files>,
}

impl Shaders {
    pub(super) fn new(root: &Path, notify: WorkerNotifier) -> Result<Self, String> {
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
            notify.clone(),
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
            root: root.to_path_buf(),
            notify,
            files: None,
        })
    }

    pub(super) fn set_root(&mut self, root: &Path) -> Result<bool, String> {
        if self.root == root {
            return Ok(false);
        }
        if let Some(files) = &mut self.files {
            files.set_root(root)?;
        }
        self.root = root.to_path_buf();
        Ok(true)
    }

    pub(super) fn sync(&mut self, capture: &CapturedChannels, references: &[ChannelReference]) -> Result<(), String> {
        let references = references.iter().copied().collect::<HashSet<_>>();
        let inputs = capture
            .channels
            .iter()
            .filter(|channel| channel.enabled && references.contains(&channel.reference))
            .filter_map(|channel| match &channel.definition.as_ref()?.source {
                ChannelSource::Shader { input } => Some(input),
                _ => None,
            })
            .collect::<Vec<_>>();
        let external = inputs
            .iter()
            .filter_map(|input| match input {
                ShaderInput::File { file: Some(file) } => Some(file.clone()),
                _ => None,
            })
            .collect::<HashSet<_>>();
        if self.files.is_none() && !external.is_empty() {
            self.files = Some(files::Files::new(&self.root, self.notify.clone())?);
        }
        if let Some(files) = &mut self.files {
            files.sync(capture.project, external);
            files.service()?;
        }
        let wanted = inputs
            .into_iter()
            .filter_map(|input| self.code(input).ok().flatten().cloned())
            .filter(|code| !code.trim().is_empty())
            .collect::<HashSet<_>>();
        self.entries.retain(|code, _| wanted.contains(code));
        for code in wanted {
            self.entries.entry(code).or_insert(None);
        }
        self.service_programs();
        Ok(())
    }

    pub(super) fn service(&mut self) -> Result<bool, String> {
        let files_changed = match &mut self.files {
            Some(files) => files.service()?,
            None => false,
        };
        Ok(self.service_programs() || files_changed)
    }

    fn service_programs(&mut self) -> bool {
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

    fn code<'a>(&'a self, input: &'a ShaderInput) -> Result<Option<&'a Arc<str>>, ChannelAvailability> {
        match input {
            ShaderInput::Embedded { code } => Ok(Some(code)),
            ShaderInput::File { file: Some(file) } => match self.files.as_ref().and_then(|files| files.get(file)) {
                Some(Ok(code)) => Ok(Some(code)),
                Some(Err(golden_assets::PreparationFailure::Missing)) => Err(ChannelAvailability::Missing),
                Some(Err(_)) => Err(ChannelAvailability::Incompatible),
                None => Ok(None),
            },
            ShaderInput::File { file: None } => Ok(None),
        }
    }

    pub(super) fn program(&self, input: &ShaderInput) -> Result<Option<&FragmentProgram>, ChannelAvailability> {
        let Some(code) = self.code(input)? else { return Ok(None) };
        if code.trim().is_empty() {
            return Err(ChannelAvailability::Unconfigured);
        }
        match self.entries.get(code).and_then(Option::as_ref) {
            Some(Ok(program)) => Ok(Some(program)),
            Some(Err(_)) => Err(ChannelAvailability::Incompatible),
            None => Ok(None),
        }
    }
}
