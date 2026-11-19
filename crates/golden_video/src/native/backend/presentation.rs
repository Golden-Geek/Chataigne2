//! Prepared dependency masks and one-shot placement wakes; no per-frame graph allocation.
use std::collections::BTreeMap;

use golden_io::{RevisionReader, RevisionSubscription, WorkerNotifier};

use crate::OutputId;
use crate::plan::{CompiledPlan, RenderOperation};

use super::super::outputs::OutputDemand;

struct Watch {
    pass: usize,
    reader: RevisionReader,
    sampled: u64,
    subscription: Option<RevisionSubscription>,
}

struct Output {
    id: OutputId,
    producer: Option<usize>,
    watches: Vec<(usize, Option<u64>)>,
    demand: OutputDemand,
}

impl Output {
    fn pending(&self, watches: &[Watch]) -> bool {
        self.watches
            .iter()
            .any(|(watch, presented)| *presented != Some(watches[*watch].reader.current()))
    }
}

pub(super) struct Presentation {
    dependencies: Vec<Vec<usize>>,
    history_dependencies: Vec<Vec<usize>>,
    pending_passes: Vec<usize>,
    watches: Vec<Watch>,
    outputs: Vec<Output>,
    passes: Vec<bool>,
    dirty: Vec<bool>,
    current: Vec<bool>,
    notify: Option<WorkerNotifier>,
}

impl Presentation {
    pub(super) fn new(plan: &CompiledPlan, notify: Option<WorkerNotifier>) -> Self {
        let producers: BTreeMap<_, _> = plan.passes().iter().enumerate().map(|(i, p)| (p.output, i)).collect();
        let dependencies: Vec<Vec<usize>> = plan
            .passes()
            .iter()
            .map(|pass| {
                pass.operation
                    .sources()
                    .filter_map(|source| producers.get(&source).copied())
                    .collect()
            })
            .collect();
        let history_dependencies = plan
            .passes()
            .iter()
            .map(|pass| {
                pass.operation
                    .history_sources()
                    .filter_map(|source| producers.get(&source).copied())
                    .collect()
            })
            .collect();
        let mut outputs: Vec<_> = plan
            .outputs()
            .iter()
            .map(|output| Output {
                id: output.id,
                producer: producers.get(&output.source).copied(),
                watches: Vec::new(),
                demand: OutputDemand::None,
            })
            .collect();
        let mut watches = Vec::new();
        for (index, pass) in plan.passes().iter().enumerate() {
            if let RenderOperation::Regions { regions, .. } = &pass.operation {
                for placement in regions.iter().filter_map(|region| region.placement.as_ref()) {
                    let reader = placement.revisions();
                    let sampled = reader.current();
                    let subscription = notify
                        .as_ref()
                        .map(|notify| reader.guard_at(sampled).subscribe(notify.clone()));
                    watches.push(Watch {
                        pass: index,
                        reader,
                        sampled,
                        subscription,
                    });
                }
            }
        }
        if !watches.is_empty() {
            let mut required = vec![false; plan.passes().len()];
            for output in &mut outputs {
                required.fill(false);
                if let Some(producer) = output.producer {
                    required[producer] = true;
                }
                for i in (0..required.len()).rev() {
                    if required[i] {
                        for dependency in &dependencies[i] {
                            required[*dependency] = true;
                        }
                    }
                }
                output.watches.extend(
                    watches
                        .iter()
                        .enumerate()
                        .filter(|(_, watch)| required[watch.pass])
                        .map(|(watch, _)| (watch, None)),
                );
            }
        }
        Self {
            dependencies,
            history_dependencies,
            pending_passes: Vec::with_capacity(plan.passes().len()),
            watches,
            outputs,
            passes: vec![false; plan.passes().len()],
            dirty: vec![false; plan.passes().len()],
            current: vec![false; plan.passes().len()],
            notify,
        }
    }

    pub(super) fn pending(&self) -> bool {
        self.pending_outputs().next().is_some()
    }

    pub(super) fn pending_outputs(&self) -> impl Iterator<Item = OutputId> + '_ {
        self.outputs
            .iter()
            .filter(|output| output.pending(&self.watches))
            .map(|output| output.id)
    }

    pub(super) fn accepted(&mut self, index: usize) {
        for (watch, presented) in &mut self.outputs[index].watches {
            *presented = Some(self.watches[*watch].sampled);
        }
    }

    /// Latch before any placement is sampled. An update during this batch invalidates its
    /// rearmed guard and remains pending for another wake, even when sampling saw it already.
    pub(super) fn begin(&mut self, presentation: bool, mut demand: impl FnMut(OutputId) -> OutputDemand) {
        self.passes.fill(false);
        self.dirty.fill(false);
        if !presentation {
            self.current.fill(false);
        }
        for output in &mut self.outputs {
            output.demand = if !presentation || output.pending(&self.watches) {
                demand(output.id)
            } else {
                OutputDemand::None
            };
            if output.demand != OutputDemand::Draw {
                continue;
            }
            if let Some(producer) = output.producer
                && !self.passes[producer]
            {
                self.passes[producer] = true;
                self.pending_passes.push(producer);
            }
            if presentation {
                for (watch, presented) in &output.watches {
                    let watch = &self.watches[*watch];
                    self.dirty[watch.pass] |= *presented != Some(watch.reader.current());
                }
            }
        }
        if presentation {
            for index in 0..self.dirty.len() {
                let changed_dependency = self.dependencies[index]
                    .iter()
                    .any(|dependency| self.dirty[*dependency]);
                self.dirty[index] |= changed_dependency;
            }
        }
        while let Some(index) = self.pending_passes.pop() {
            self.current[index] = true;
            // Delayed edges can point forward or form cycles. Each producer joins the mask once.
            for &dependency in self.dependencies[index].iter().chain(&self.history_dependencies[index]) {
                // A skipped branch may still contain pixels from an older content batch.
                if !self.passes[dependency] && (self.dirty[dependency] || !self.current[dependency]) {
                    self.passes[dependency] = true;
                    self.pending_passes.push(dependency);
                }
            }
        }
        for watch in &mut self.watches {
            if self.passes[watch.pass] {
                watch.sampled = watch.reader.current();
                watch.subscription = self
                    .notify
                    .as_ref()
                    .map(|notify| watch.reader.guard_at(watch.sampled).subscribe(notify.clone()));
            }
        }
    }

    pub(super) fn pass(&self, index: usize) -> bool {
        self.passes[index]
    }
    pub(super) fn output(&self, index: usize) -> OutputDemand {
        self.outputs[index].demand
    }
}
