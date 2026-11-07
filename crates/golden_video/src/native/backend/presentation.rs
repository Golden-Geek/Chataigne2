//! Prepared dependency masks and one-shot placement wakes; no per-frame graph allocation.
use std::collections::BTreeMap;

use golden_io::{RevisionReader, RevisionSubscription, WorkerNotifier};

use crate::plan::{CompiledPlan, RenderOperation};

struct Watch {
    pass: usize,
    output: crate::OutputId,
    reader: RevisionReader,
    sampled: u64,
    presented: Option<u64>,
    subscription: Option<RevisionSubscription>,
}

pub(super) struct Presentation {
    dependencies: Vec<Vec<usize>>,
    watches: Vec<Watch>,
    output_passes: Vec<Option<usize>>,
    passes: Vec<bool>,
    outputs: Vec<bool>,
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
        let output_passes: Vec<_> = plan
            .outputs()
            .iter()
            .map(|output| producers.get(&output.source).copied())
            .collect();
        let mut reachable = vec![false; plan.passes().len()];
        for index in output_passes.iter().flatten() {
            reachable[*index] = true;
        }
        for i in (0..reachable.len()).rev() {
            if reachable[i] {
                for dependency in &dependencies[i] {
                    reachable[*dependency] = true;
                }
            }
        }
        let mut watches = Vec::new();
        let ancestors: Vec<_> = output_passes
            .iter()
            .map(|producer| {
                let mut required = vec![false; plan.passes().len()];
                if let Some(producer) = producer {
                    required[*producer] = true;
                }
                for i in (0..required.len()).rev() {
                    if required[i] {
                        for dependency in &dependencies[i] {
                            required[*dependency] = true;
                        }
                    }
                }
                required
            })
            .collect();
        for (index, pass) in plan.passes().iter().enumerate().filter(|(i, _)| reachable[*i]) {
            if let RenderOperation::Regions { regions, .. } = &pass.operation {
                for placement in regions.iter().filter_map(|region| region.placement.as_ref()) {
                    let reader = placement.revisions();
                    let sampled = reader.current();
                    let subscription = notify
                        .as_ref()
                        .map(|notify| reader.guard_at(sampled).subscribe(notify.clone()));
                    watches.push(Watch {
                        pass: index,
                        output: plan.outputs()[ancestors
                            .iter()
                            .position(|required| required[index])
                            .expect("reachable presentation output")]
                        .id,
                        reader,
                        sampled,
                        presented: None,
                        subscription,
                    });
                }
            }
        }
        Self {
            dependencies,
            watches,
            output_passes,
            passes: vec![false; plan.passes().len()],
            outputs: vec![false; plan.outputs().len()],
            notify,
        }
    }

    pub(super) fn pending(&self) -> bool {
        self.pending_outputs().next().is_some()
    }

    pub(super) fn pending_outputs(&self) -> impl Iterator<Item = crate::OutputId> + '_ {
        self.watches
            .iter()
            .filter(|watch| watch.presented != Some(watch.reader.current()))
            .map(|watch| watch.output)
    }

    pub(super) fn accepted(&mut self, output: crate::OutputId) {
        for watch in &mut self.watches {
            if watch.output == output {
                watch.presented = Some(watch.sampled);
            }
        }
    }

    /// Latch before any placement is sampled. An update during this batch invalidates its
    /// rearmed guard and remains pending for another wake, even when sampling saw it already.
    pub(super) fn begin(&mut self, presentation: bool) {
        self.passes.fill(!presentation);
        for watch in &self.watches {
            if watch.presented != Some(watch.reader.current()) {
                self.passes[watch.pass] = true;
            }
        }
        if presentation {
            for index in 0..self.passes.len() {
                let changed_dependency = self.dependencies[index]
                    .iter()
                    .any(|dependency| self.passes[*dependency]);
                self.passes[index] |= changed_dependency;
            }
        }
        for (index, producer) in self.output_passes.iter().enumerate() {
            self.outputs[index] = !presentation || producer.is_some_and(|pass| self.passes[pass]);
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
    pub(super) fn output(&self, index: usize) -> bool {
        self.outputs[index]
    }
}
