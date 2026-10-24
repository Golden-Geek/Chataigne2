use super::*;
use golden_core::edit::NodeReadGuard;
use golden_io::{PayloadPool, RetainedPayload, RetirementPool, RevisionGuard, WorkerNotifier};
use std::{
    num::NonZeroUsize,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};

pub(super) const MAX_ENDPOINTS: usize = 4096;
pub(super) const MAX_PAIRS: usize = 1024 * 1024;
pub(super) const MAX_LABEL_BYTES: usize = 512;
pub(super) const MAX_ENDPOINT_CHILDREN: usize = 16;

#[cfg(test)]
#[derive(Default)]
pub(super) struct TestControl {
    pub entered: AtomicBool,
    pub released: AtomicBool,
    pub panic: AtomicBool,
}

pub(super) struct Prepared {
    pub config: SpatializerConfig,
    pub values: Vec<f64>,
    pub input_guard: NodeReadGuard,
    pub source_decls: HashSet<String>,
    pub target_decls: HashSet<String>,
}

impl Prepared {
    pub fn value(&self, outer: usize, inner: usize) -> f64 {
        let (source, target) = match self.config.value_layout {
            SpatializerValueLayout::SourceCentric => (outer, inner),
            SpatializerValueLayout::TargetCentric => (inner, outer),
        };
        self.values[source * self.config.targets.len() + target]
    }
    pub fn outer(&self) -> &[SpatializerEndpointConfig] {
        match self.config.value_layout {
            SpatializerValueLayout::SourceCentric => &self.config.sources,
            SpatializerValueLayout::TargetCentric => &self.config.targets,
        }
    }
    pub fn inner(&self) -> &[SpatializerEndpointConfig] {
        match self.config.value_layout {
            SpatializerValueLayout::SourceCentric => &self.config.targets,
            SpatializerValueLayout::TargetCentric => &self.config.sources,
        }
    }
}

#[derive(Debug)]
pub(super) enum Failure {
    Invalid(String),
    Retry(&'static str),
}
impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Invalid(message)
    }
}
impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        Self::Invalid(message.into())
    }
}
#[derive(Clone, Copy)]
pub(super) struct Roots {
    pub parameters: NodeId,
    pub sources: NodeId,
    pub targets: NodeId,
}
type ResultSlot = Arc<Mutex<Option<Result<RetainedPayload<Prepared>, Failure>>>>;
pub(super) struct Job {
    result: ResultSlot,
    running: Arc<AtomicBool>,
}
impl Job {
    pub fn take(&self) -> Option<Result<RetainedPayload<Prepared>, Failure>> {
        self.result.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
    pub fn running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}

pub(super) fn workers() -> &'static RetirementPool {
    static WORKERS: OnceLock<RetirementPool> = OnceLock::new();
    WORKERS.get_or_init(|| RetirementPool::new(NonZeroUsize::new(4).unwrap()))
}
pub(super) fn pool() -> Result<PayloadPool, String> {
    static POOL: Mutex<Option<PayloadPool>> = Mutex::new(None);
    let mut pool = POOL.lock().unwrap_or_else(|e| e.into_inner());
    if pool.is_none() {
        *pool = Some(PayloadPool::new("spatializer-prepared", 64, 128 * 1024 * 1024).map_err(|e| e.to_string())?);
    }
    Ok(pool.as_ref().unwrap().clone())
}

/// Constant-time count admission precedes leasing, copying endpoint labels,
/// constructing worker inputs, allocating output storage, or starting a thread.
pub(super) fn admit(sources: usize, targets: usize) -> Result<usize, String> {
    let pairs = sources.checked_mul(targets).ok_or("Spatializer matrix size overflow")?;
    if sources > MAX_ENDPOINTS || targets > MAX_ENDPOINTS || pairs > MAX_PAIRS {
        return Err("Spatializer supports up to 4096 sources, 4096 targets and 1048576 pairs".into());
    }
    // Includes endpoint strings/sets/capacity rounding, dense values, Delaunay
    // scratch and per-source temporary weights. Scratch never scales by pairs.
    Ok(64 * 1024 + (sources + targets) * 4096 + pairs * size_of::<f64>())
}

pub(super) fn start(
    snapshot: &Arc<ProcessTreeSnapshot>,
    roots: Roots,
    owner: &RetirementPool,
    generation: RevisionGuard,
    notifier: WorkerNotifier,
    #[cfg(test)] control: Option<Arc<TestControl>>,
) -> Result<Job, String> {
    let Roots {
        parameters,
        sources,
        targets,
    } = roots;
    let weight = admit(
        snapshot.child_ids_slice(sources).len(),
        snapshot.child_ids_slice(targets).len(),
    )?;
    let reservation = pool()?
        .try_reserve(weight)
        .map_err(|_| "Spatializer preparation lifetime capacity is full")?;
    let process = workers()
        .try_reserve()
        .map_err(|_| "Spatializer process worker capacity is full")?;
    let module = owner
        .try_reserve()
        .map_err(|_| "Spatializer previous worker is still retiring")?;
    let source = snapshot.try_lease().map_err(|e| e.to_string())?;
    let result: ResultSlot = Arc::default();
    let running = Arc::new(AtomicBool::new(true));
    let publish = result.clone();
    let active = running.clone();
    process
        .spawn(
            "chataigne-spatializer",
            (module, source, reservation),
            move |(module, source, reservation)| {
                let computed = catch_unwind(AssertUnwindSafe(|| {
                    #[cfg(test)]
                    if let Some(control) = control {
                        control.entered.store(true, Ordering::Release);
                        while !control.released.load(Ordering::Acquire) {
                            std::thread::sleep(std::time::Duration::from_millis(1));
                        }
                        assert!(
                            !control.panic.load(Ordering::Acquire),
                            "injected Spatializer worker failure"
                        );
                    }
                    if !generation.is_current() {
                        return Err(Failure::Retry("Spatializer preparation superseded"));
                    }
                    let config = capture(&source, parameters, sources, targets)?;
                    let values = math::dense_values(&config, || !generation.is_current())
                        .ok_or(Failure::Retry("Spatializer preparation superseded"))?;
                    let prepared = Prepared {
                        source_decls: config.sources.iter().map(|e| e.value_decl_id.clone()).collect(),
                        target_decls: config.targets.iter().map(|e| e.value_decl_id.clone()).collect(),
                        config,
                        values,
                        input_guard: NodeReadGuard::capture_subtree(&source, parameters)
                            .ok_or("Spatializer parameters disappeared")?,
                    };
                    reservation
                        .retain(prepared, weight)
                        .map_err(|_| Failure::Retry("Spatializer prepared result exceeds its reservation"))
                }))
                .unwrap_or_else(|_| Err(Failure::Retry("Spatializer preparation worker panicked")));
                *publish.lock().unwrap_or_else(|e| e.into_inner()) = Some(computed);
                // Wake only after input cleanup and the module slot have been released.
                drop(source);
                drop(module);
                active.store(false, Ordering::Release);
                notifier.notify();
            },
        )
        .map_err(|e| e.to_string())?;
    Ok(Job { result, running })
}

pub(super) fn controls(
    snapshot: &ProcessTreeSnapshot,
    parameters: NodeId,
) -> Result<(SpatializerDimension, SpatializerMode, SpatializerValueLayout), String> {
    let token = |key| -> Result<&str, String> {
        let value = snapshot
            .find_child_by_decl_id(parameters, key)
            .and_then(|id| snapshot.node(id))
            .and_then(|node| node.param_value.as_ref());
        match value {
            Some(ParamValue::Enum(value)) if value.len() <= 64 => Ok(value.as_str()),
            _ => Err("Spatializer requires bounded Dimensions, Mode and Value Layout enum parameters".into()),
        }
    };
    Ok((
        parse_spatializer_dimension(token("dimensions")?),
        parse_spatializer_mode(token("mode")?),
        parse_spatializer_value_layout(token("value_layout")?),
    ))
}

fn capture(
    snapshot: &ProcessTreeSnapshot,
    parameters: NodeId,
    sources: NodeId,
    targets: NodeId,
) -> Result<SpatializerConfig, Failure> {
    admit(
        snapshot.child_ids_slice(sources).len(),
        snapshot.child_ids_slice(targets).len(),
    )?;
    let (dimension, mode, value_layout) = controls(snapshot, parameters)?;
    let capture_items = |parent, source| -> Result<Vec<SpatializerEndpointConfig>, Failure> {
        let mut endpoints = Vec::with_capacity(snapshot.child_ids_slice(parent).len());
        for id in snapshot.child_ids_slice(parent) {
            let Some(node) = snapshot.node(*id) else { continue };
            if node.node_type
                != if source {
                    SpatializerSourceItem::NODE_TYPE
                } else {
                    SpatializerTargetItem::NODE_TYPE
                }
            {
                continue;
            }
            if node.label.len() > MAX_LABEL_BYTES || snapshot.child_ids_slice(*id).len() > MAX_ENDPOINT_CHILDREN {
                return Err(
                    "Spatializer endpoints support labels up to 512 bytes and at most 16 direct children".into(),
                );
            }
            if !endpoint_ready(snapshot, *id, dimension, mode, source) {
                return Err(Failure::Retry("Spatializer endpoint normalization required"));
            }
            let prefix = if source {
                VALUE_SOURCE_DECL_PREFIX
            } else {
                VALUE_TARGET_DECL_PREFIX
            };
            endpoints.push(spatializer_endpoint_config(node, snapshot, *id, dimension, prefix).unwrap());
        }
        Ok(endpoints)
    };
    Ok(SpatializerConfig {
        dimension,
        mode,
        value_layout,
        sources: capture_items(sources, true)?,
        targets: capture_items(targets, false)?,
    })
}

fn endpoint_ready(
    snapshot: &ProcessTreeSnapshot,
    id: NodeId,
    dimension: SpatializerDimension,
    mode: SpatializerMode,
    source: bool,
) -> bool {
    let position = direct_child_ids_by_decl(snapshot, id, dimension.position_decl_id());
    let stale = direct_child_ids_by_decl(snapshot, id, dimension.stale_position_decl_id());
    let float_ready = |decl: &str, required: bool| {
        let children = direct_child_ids_by_decl(snapshot, id, decl);
        if !required {
            return children.is_empty();
        }
        children.len() == 1 && matches!(child_param_value(snapshot, children[0]), Some(ParamValue::Float(_)))
    };
    position.len() == 1
        && stale.is_empty()
        && child_param_value(snapshot, position[0])
            .is_some_and(|value| position_value_matches_dimension(value, dimension))
        && float_ready(
            RADIUS_DECL_ID,
            if source {
                mode.source_radius_required()
            } else {
                mode.target_radius_required()
            },
        )
        && (source || float_ready(FREEZE_RADIUS_DECL_ID, mode.target_freeze_radius_required()))
}
