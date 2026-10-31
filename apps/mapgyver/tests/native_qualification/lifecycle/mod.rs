//! Renderer-only Q05 lifecycle operations. Full app/Matrix/WebView qualification stays separate.
mod contract;
mod owner;
#[cfg(test)]
mod tests;

use golden_io::{BoundedWorker, RetirementPool, WorkerContext, WorkerLimits, WorkerReceive};
use golden_video::{
    Extent2D,
    native::{AdapterId, query_adapter_memory},
};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{self, Error, Write},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};
use uuid::Uuid;

use super::{
    config::{Arguments, FIXTURE_MAX_SECONDS},
    roles::{self, OwnedRole},
};
use contract::{BASELINE_CYCLE, CYCLES, DRAIN_SECONDS, EDIT_INTERVAL_MS, EDITS_PER_CYCLE, SPACING_MS};
use owner::Owner;

struct Config {
    directory: PathBuf,
    adapter_index: usize,
    cycles: u32,
    spacing_ms: u64,
    diagnostic: bool,
}
impl Config {
    fn read(mut args: Arguments) -> io::Result<Self> {
        let value = Self {
            directory: args.required("directory")?.into(),
            adapter_index: args.number("adapter", "0")?,
            cycles: args.number("cycles", "100")?,
            spacing_ms: args.number("spacing-ms", "2000")?,
            diagnostic: args.take("diagnostic", "false").parse::<bool>().map_err(Error::other)?,
        };
        args.done()?;
        if !(12..=CYCLES).contains(&value.cycles)
            || !value.cycles.is_multiple_of(2)
            || value.spacing_ms > 10000
            || (!value.diagnostic && (value.cycles != CYCLES || value.spacing_ms < SPACING_MS))
        {
            return Err(Error::other(
                "lifecycle qualification requires 100 even cycles and >=2000 ms spacing; shorter bounded controls require --diagnostic true",
            ));
        }
        Ok(value)
    }
}

fn write_new(path: &Path, value: &Value) -> io::Result<()> {
    let mut output = OpenOptions::new().write(true).create_new(true).open(path)?;
    serde_json::to_writer_pretty(&mut output, value).map_err(Error::other)?;
    output.write_all(b"\n")
}

fn row(output: &mut std::fs::File, value: Value) -> io::Result<()> {
    serde_json::to_writer(&mut *output, &value).map_err(Error::other)?;
    output.write_all(b"\n")?;
    output.flush()
}

fn pace(context: &WorkerContext<(), ()>, deadline: Instant) -> io::Result<()> {
    match context.receive_until(deadline) {
        WorkerReceive::Deadline => Ok(()),
        _ => Err(Error::other("lifecycle timing worker cancelled")),
    }
}

fn fixture(
    config: &Config,
    role: &str,
    name: &str,
    extent: Extent2D,
    source: u32,
    cycle: u32,
) -> io::Result<OwnedRole> {
    let label = format!("{role}-{source}-{cycle:03}");
    let directory = config.directory.join(&label);
    fs::create_dir(&directory)?;
    let arguments = vec![
        "--role".into(),
        role.into(),
        "--adapter".into(),
        config.adapter_index.to_string(),
        "--directory".into(),
        directory.to_string_lossy().into_owned(),
        "--name".into(),
        name.into(),
        "--width".into(),
        extent.width().to_string(),
        "--height".into(),
        extent.height().to_string(),
        "--source-id".into(),
        source.to_string(),
        "--instance".into(),
        Uuid::new_v4().to_string(),
        "--max-seconds".into(),
        FIXTURE_MAX_SECONDS.to_string(),
        "--trace-capacity".into(),
        "65536".into(),
    ];
    OwnedRole::spawn(&std::env::current_exe()?, &arguments, &directory, "process")
}

fn finish(child: &mut OwnedRole, output: &mut std::fs::File) -> io::Result<()> {
    let result = child.finish()?;
    row(output, json!({"kind":"child_exit","result":result}))?;
    if result["success"] != true {
        return Err(Error::other("owned lifecycle fixture did not exit normally"));
    }
    Ok(())
}

pub fn run(args: Arguments) -> io::Result<()> {
    let config = Config::read(args)?;
    fs::create_dir(&config.directory)?;
    let directory = config.directory.clone();
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (done, result) = mpsc::sync_channel(1);
    let worker = BoundedWorker::<(), ()>::spawn(
        "native-lifecycle-timing",
        &pool,
        WorkerLimits {
            command_items: 1,
            command_weight: 1,
            event_items: 1,
            event_weight: 1,
        },
        move |mut context| {
            let outcome = context
                .enable_precise_deadlines()
                .and_then(|()| execute(config, &context));
            let _ = done.send(outcome);
        },
    )
    .map_err(Error::other)?;
    let outcome = result.recv().map_err(Error::other)?;
    drop(worker);
    if !pool.wait_for_idle(Duration::from_secs(10)) {
        return Err(Error::other("lifecycle timing worker did not retire"));
    }
    if let Err(error) = &outcome {
        let path = directory.join("lifecycle-summary.json");
        if !path.exists() {
            write_new(
                &path,
                &json!({"status":"FAILED_NATIVE","qualified":false,"error":error.to_string(),"scope":"renderer_lifecycle_growth"}),
            )?;
        }
    }
    outcome
}

fn create_owner(
    adapter: AdapterId,
    name: &str,
    ceiling: u64,
    sources: &[String; 2],
    extent: Extent2D,
) -> io::Result<Owner> {
    let mut owner = Owner::new(adapter, name, ceiling)?;
    owner.install(owner::plan(sources, extent, 0)?)?;
    owner.streaming(extent)?;
    Ok(owner)
}

fn execute(config: Config, context: &WorkerContext<(), ()>) -> io::Result<()> {
    let adapter = roles::selected(config.adapter_index)?;
    let memory = query_adapter_memory(adapter).map_err(Error::other)?;
    let ceiling = memory.admission_limit(512 * 1024 * 1024).map_err(Error::other)?;
    let id = Uuid::new_v4();
    let names = [format!("GoldenLifecycle-{id}-A"), format!("GoldenLifecycle-{id}-B")];
    let output_name = format!("GoldenLifecycle-{id}-Output");
    let full = Extent2D::new(1920, 1080).unwrap();
    let small = Extent2D::new(1280, 720).unwrap();
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(config.directory.join("lifecycle.jsonl"))?;
    let mut source = fixture(&config, "source", &names[0], full, 1, 0)?;
    let mut healthy = fixture(&config, "source", &names[1], small, 2, 0)?;
    let mut consumer = fixture(&config, "consumer", &output_name, full, 3, 0)?;
    row(
        &mut output,
        json!({"kind":"config","process_id":std::process::id(),"adapter":adapter,"adapter_memory":memory,
        "cycles":config.cycles,"spacing_ms":config.spacing_ms,"diagnostic":config.diagnostic,"source_names":names,"qpc_frequency":roles::frequency()?,
        "output_name":output_name,"initial_fixture_pids":[source.id(),healthy.id(),consumer.id()],
        "baseline_cycle":BASELINE_CYCLE,"drain_seconds":DRAIN_SECONDS,"edits_per_cycle":EDITS_PER_CYCLE,
        "fixture_max_seconds":FIXTURE_MAX_SECONDS,"spacing_semantics":"pause after lifecycle work, before checkpoint/next cycle",
        "scope":"renderer lifecycle; two native host windows; no WebView or Matrix lifecycle claim"}),
    )?;
    let mut owner = create_owner(adapter, &output_name, ceiling, &names, full)?;
    owner.settle(Duration::from_secs(2))?;
    let mut baseline = None;
    let mut maximum_private = 0;
    let mut edits = 0;
    let mut superseded = 0;
    let mut installed = 0;
    let began = Instant::now();
    for cycle in 1..=config.cycles {
        let cycle_began = Instant::now();
        let extent = if cycle.is_multiple_of(2) { full } else { small };
        let before = owner
            .source(2)
            .ok_or_else(|| Error::other("healthy source absent before restart"))?;
        let frame_before = owner.native.snapshot().accepted_outputs;
        finish(&mut source, &mut output)?;
        pace(context, Instant::now() + Duration::from_millis(100))?;
        owner.wait("healthy channel advances while peer sender is absent", |s| {
            s.source(2)
                .is_some_and(|v| v.binding == before.binding && v.renderer_copies > before.renderer_copies)
                && s.native.snapshot().accepted_outputs > frame_before
        })?;
        source = fixture(&config, "source", &names[0], extent, 1, cycle)?;
        row(
            &mut output,
            json!({"kind":"child_started","cycle":cycle,"pid":source.id()}),
        )?;
        owner.install(owner::plan(&names, extent, 0)?)?;
        owner.streaming(extent)?;
        owner.replace_project()?;
        owner.install(owner::plan(&names, extent, 0)?)?;
        owner.streaming(extent)?;
        let burst_began = Instant::now();
        let burst_begin_qpc = roles::qpc()?;
        let mut edit_receipts_qpc = Vec::with_capacity(EDITS_PER_CYCLE as usize);
        for edit in 0..EDITS_PER_CYCLE {
            owner.publish(owner::plan(&names, extent, edit)?)?;
            edit_receipts_qpc.push(roles::qpc()?);
            edits += 1;
            pace(
                context,
                burst_began + Duration::from_millis(u64::from(edit + 1) * EDIT_INTERVAL_MS),
            )?;
        }
        let burst_end_qpc = roles::qpc()?;
        owner.settle(Duration::ZERO)?;
        owner.install(owner::plan(&names, extent, 0)?)?;
        superseded += owner.superseded;
        installed += owner.installed;
        let retirement = owner.retire()?;
        drop(owner);
        owner = create_owner(adapter, &output_name, ceiling, &names, extent)?;
        let post_cycle_settle = contract::post_cycle_settle(cycle, config.cycles, config.spacing_ms);
        let settle_begin_qpc = roles::qpc()?;
        owner.settle(post_cycle_settle)?;
        let settle_end_qpc = roles::qpc()?;
        let counters = owner.counters()?;
        if cycle == BASELINE_CYCLE {
            baseline = Some(counters);
        }
        if cycle > BASELINE_CYCLE {
            maximum_private = maximum_private.max(counters.private_bytes);
        }
        row(
            &mut output,
            json!({"kind":"cycle","cycle":cycle,"observed_qpc":roles::qpc()?,
            "elapsed_seconds":began.elapsed().as_secs_f64(),"cycle_seconds":cycle_began.elapsed().as_secs_f64(),
            "source_extent":[extent.width(),extent.height()],"source_resize_restarts":cycle,"project_replacements":cycle,
            "output_remove_recreates":cycle,"superseding_edits":edits,"superseded_outcomes":superseded,
            "installed_outcomes":installed,"healthy_source_advanced_while_peer_absent":true,
            "post_cycle_settle_begin_qpc":settle_begin_qpc,"post_cycle_settle_end_qpc":settle_end_qpc,
            "post_cycle_settle_requested_seconds":post_cycle_settle.as_secs_f64(),
            "edit_burst_begin_qpc":burst_begin_qpc,"edit_burst_end_qpc":burst_end_qpc,"edit_receipts_qpc":edit_receipts_qpc,
            "output_retirement_ms":retirement.as_secs_f64()*1000.0,"counters":counters}),
        )?;
    }
    let baseline = baseline.ok_or_else(|| Error::other("warm baseline missing"))?;
    let final_state = owner.counters()?;
    let verdict = contract::evaluate(
        baseline,
        final_state,
        maximum_private,
        config.cycles,
        config.spacing_ms,
        edits,
        superseded,
    );
    let observed = verdict.observed_within_target;
    let eligible = verdict.eligible && !config.diagnostic;
    let shutdown = owner.retire()?;
    drop(owner);
    finish(&mut source, &mut output)?;
    finish(&mut healthy, &mut output)?;
    finish(&mut consumer, &mut output)?;
    write_new(
        &config.directory.join("lifecycle-summary.json"),
        &json!({
            "status":if !eligible {"UNAVAILABLE"} else if observed {"PASSED_NATIVE"} else {"FAILED_NATIVE"},
            "scope":"renderer_handle_private_growth","full_renderer_qualified":false,"full_app_q05":false,
            "adapter":adapter,"process_id":std::process::id(),"cycles":config.cycles,"spacing_ms":config.spacing_ms,
            "source_resize_restarts":config.cycles,"project_replacements":config.cycles,"output_remove_recreates":config.cycles,
            "superseding_edits":edits,"superseded_outcomes":superseded,"baseline":baseline,"final_state":final_state,
        "maximum_post_baseline_checkpoint_private_bytes":maximum_private,"verdict":verdict,"shutdown_ms":shutdown.as_secs_f64()*1000.0,
        "post_shutdown_process":golden_video::native::query_process_resources(std::process::id()).map_err(Error::other)?,
        "sampling":"stable checkpoints after every completed cycle; transient creation peaks are not established",
            "limits":["logical reservations are not physical GPU allocation identities","driver/SDK physical texture allocation identity remains unavailable","WebView/Matrix/full-app lifecycle belongs to M09","window deadline/GPU critical-path/queue/recovery gates are not closed"]
        }),
    )?;
    if !observed {
        return Err(Error::other("renderer lifecycle growth budget failed"));
    }
    Ok(())
}
