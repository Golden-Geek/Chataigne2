use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Error, Write};
use std::num::NonZeroUsize;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use golden_video::native::fixtures::{
    CardStamp, FixtureConsumer, FixtureConsumerConfig, FixtureSource, FixtureSourceConfig,
};
use golden_video::native::{
    AdapterId, DiagnosticEvent, DiagnosticsConfig, adapters, query_adapter_memory, query_process_resources,
};
use golden_video::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};
use serde_json::{Value, json};
use uuid::Uuid;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

use super::config::{Arguments, RunConfig};
use super::host::{self, HostConfig};

fn selected(index: usize) -> io::Result<AdapterId> {
    adapters()
        .map_err(Error::other)?
        .get(index)
        .filter(|adapter| !adapter.software)
        .map(|adapter| adapter.id)
        .ok_or_else(|| Error::other("selected hardware adapter is absent"))
}

pub fn qpc() -> io::Result<u64> {
    let mut value = 0_i64;
    // SAFETY: QPC writes only to this initialized local coordinate.
    unsafe { QueryPerformanceCounter(&mut value) }.map_err(Error::other)?;
    u64::try_from(value)
        .ok()
        .filter(|value| *value != 0)
        .ok_or_else(|| Error::other("invalid QPC"))
}

fn frequency() -> io::Result<u64> {
    let mut value = 0_i64;
    // SAFETY: the API writes only to this initialized local frequency.
    unsafe { QueryPerformanceFrequency(&mut value) }.map_err(Error::other)?;
    u64::try_from(value)
        .ok()
        .filter(|value| *value != 0)
        .ok_or_else(|| Error::other("invalid QPC frequency"))
}

fn writer(path: &Path) -> io::Result<BufWriter<File>> {
    Ok(BufWriter::new(
        OpenOptions::new().write(true).create_new(true).open(path)?,
    ))
}

fn row(writer: &mut BufWriter<File>, value: Value) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, &value).map_err(Error::other)?;
    writer.write_all(b"\n")
}

fn resource_row(writer: &mut BufWriter<File>, adapter: AdapterId) -> io::Result<()> {
    row(
        writer,
        json!({"kind":"resources","qpc":qpc()?,
        "process":query_process_resources(std::process::id()).ok(),
        "adapter_memory":query_adapter_memory(adapter).ok()}),
    )
}

fn stop_receiver() -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut line = String::new();
        let _ = io::stdin().lock().read_line(&mut line);
        let _ = tx.try_send(());
    });
    rx
}

fn diagnostic_rows(writer: &mut BufWriter<File>, values: &mut Vec<DiagnosticEvent>) -> io::Result<()> {
    for event in values.drain(..) {
        row(writer, json!({"kind":"diagnostic","event":event}))?;
    }
    Ok(())
}

fn descriptor(width: u32, height: u32, alpha_mode: AlphaMode) -> io::Result<TextureDescriptor> {
    if width > 16384 || height > 16384 {
        return Err(Error::other("fixture extent exceeds native bound"));
    }
    Ok(TextureDescriptor {
        extent: Extent2D::new(width, height).map_err(Error::other)?,
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode,
    })
}

struct FixtureArguments {
    adapter: AdapterId,
    name: String,
    width: u32,
    height: u32,
    instance: Uuid,
    directory: PathBuf,
    source_id: u32,
    maximum: Duration,
    capacity: NonZeroUsize,
}

impl FixtureArguments {
    fn read(args: &mut Arguments) -> io::Result<Self> {
        let result = Self {
            adapter: selected(args.number("adapter", "0")?)?,
            name: args.required("name")?,
            width: args.number("width", "1920")?,
            height: args.number("height", "1080")?,
            instance: args.required("instance")?.parse().map_err(Error::other)?,
            directory: PathBuf::from(args.required("directory")?),
            source_id: args.number("source-id", "1")?,
            maximum: Duration::from_secs(args.number("max-seconds", "240")?),
            capacity: NonZeroUsize::new(args.number("trace-capacity", "65536")?)
                .ok_or_else(|| Error::other("zero trace capacity"))?,
        };
        if result.maximum.is_zero() || result.maximum > Duration::from_secs(900) || result.capacity.get() > 65536 {
            return Err(Error::other("fixture duration/capacity out of bounds"));
        }
        Ok(result)
    }
}

pub fn source(mut args: Arguments) -> io::Result<()> {
    let config = FixtureArguments::read(&mut args)?;
    args.done()?;
    let mut output = writer(&config.directory.join(format!("source-{}.jsonl", config.source_id)))?;
    let mut fixture = FixtureSource::new(FixtureSourceConfig {
        adapter: config.adapter,
        descriptor: descriptor(config.width, config.height, AlphaMode::Straight)?,
        source_id: config.source_id,
        instance: config.instance,
        sender_name: config.name,
        diagnostics: DiagnosticsConfig {
            capacity: config.capacity,
        },
    })
    .map_err(Error::other)?;
    let stop = stop_receiver();
    let started = Instant::now();
    let mut attempted = 0_u64;
    let mut missed = 0_u64;
    let mut drain_at = started;
    let mut resource_at = started;
    let mut trace = Vec::with_capacity(4096);
    row(
        &mut output,
        json!({"kind":"source_config","qpc_frequency":frequency()?,"pid":std::process::id(),"source":fixture.snapshot()}),
    )?;
    println!("{}", json!({"kind":"ready","pid":std::process::id()}));
    io::stdout().flush()?;
    while started.elapsed() < config.maximum && stop.try_recv().is_err() {
        fixture.poll().map_err(Error::other)?;
        let due = u64::try_from(started.elapsed().as_nanos() * 60 / 1_000_000_000 + 1).map_err(Error::other)?;
        if due > attempted {
            missed = missed.saturating_add(due - attempted - 1);
            attempted = due;
            let stamp = CardStamp {
                source: config.source_id,
                frame: due,
                qpc: qpc()?,
            };
            if let Some(submission) = fixture.submit(stamp).map_err(Error::other)? {
                row(
                    &mut output,
                    json!({"kind":"source_frame","submission":submission,"marker_words":stamp.words()}),
                )?;
            }
        }
        if Instant::now() >= drain_at {
            fixture.drain_diagnostics(&mut trace, NonZeroUsize::new(4096).unwrap());
            diagnostic_rows(&mut output, &mut trace)?;
            drain_at = Instant::now() + Duration::from_millis(25);
        }
        if Instant::now() >= resource_at {
            resource_row(&mut output, config.adapter)?;
            resource_at = Instant::now() + Duration::from_secs(1);
        }
        thread::sleep(Duration::from_millis(1));
    }
    let drain_started = Instant::now();
    fixture.finish().map_err(Error::other)?;
    while fixture.drain_diagnostics(&mut trace, NonZeroUsize::new(4096).unwrap()) != 0 {
        diagnostic_rows(&mut output, &mut trace)?;
    }
    row(
        &mut output,
        json!({"kind":"source_final","qpc":qpc()?,"source":fixture.snapshot(),
        "diagnostics":fixture.diagnostics_snapshot(),"missed_fixture_deadlines":missed,
        "drain_ns":drain_started.elapsed().as_nanos().to_string()}),
    )?;
    drop(fixture);
    resource_row(&mut output, config.adapter)?;
    output.flush()?;
    println!("{}", json!({"kind":"done","pid":std::process::id()}));
    Ok(())
}

pub fn consumer(mut args: Arguments) -> io::Result<()> {
    let config = FixtureArguments::read(&mut args)?;
    let max_frames: u32 = args.number("max-frames", "65536")?;
    args.done()?;
    let mut output = writer(&config.directory.join("consumer.jsonl"))?;
    let mut fixture = FixtureConsumer::new(FixtureConsumerConfig {
        adapter: config.adapter,
        sender_name: config.name,
        frame_count: golden_video::plan::SpoutFrameCount::VersionedV1,
        descriptor: descriptor(config.width, config.height, AlphaMode::Straight)?,
        max_frames,
        instance: config.instance,
        diagnostics: DiagnosticsConfig {
            capacity: config.capacity,
        },
    })
    .map_err(Error::other)?;
    let stop = stop_receiver();
    let started = Instant::now();
    let mut trace = Vec::with_capacity(4096);
    let mut drain_at = started;
    let mut resource_at = started;
    row(
        &mut output,
        json!({"kind":"consumer_config","pid":std::process::id(),"qpc_frequency":frequency()?,"consumer":fixture.snapshot()}),
    )?;
    println!("{}", json!({"kind":"ready","pid":std::process::id()}));
    io::stdout().flush()?;
    while started.elapsed() < config.maximum && stop.try_recv().is_err() {
        fixture.tick().map_err(Error::other)?;
        if Instant::now() >= drain_at {
            fixture.drain_diagnostics(&mut trace, NonZeroUsize::new(4096).unwrap());
            diagnostic_rows(&mut output, &mut trace)?;
            drain_at = Instant::now() + Duration::from_millis(25);
        }
        if Instant::now() >= resource_at {
            resource_row(&mut output, config.adapter)?;
            resource_at = Instant::now() + Duration::from_secs(1);
        }
        thread::sleep(Duration::from_millis(1));
    }
    let readback_started = qpc()?;
    let frames = fixture.finish().map_err(Error::other)?;
    for frame in frames {
        row(&mut output, json!({"kind":"consumer_frame","frame":frame}))?;
    }
    while fixture.drain_diagnostics(&mut trace, NonZeroUsize::new(4096).unwrap()) != 0 {
        diagnostic_rows(&mut output, &mut trace)?;
    }
    row(
        &mut output,
        json!({"kind":"consumer_final","qpc":qpc()?,"final_readback_begin_qpc":readback_started,
        "consumer":fixture.snapshot(),"diagnostics":fixture.diagnostics_snapshot()}),
    )?;
    drop(fixture);
    resource_row(&mut output, config.adapter)?;
    output.flush()?;
    println!("{}", json!({"kind":"done","pid":std::process::id()}));
    Ok(())
}

struct OwnedRole {
    child: Child,
    reader: Option<thread::JoinHandle<()>>,
}

impl OwnedRole {
    fn spawn(executable: &Path, values: &[String], directory: &Path, label: &str) -> io::Result<Self> {
        let stdout_log = writer(&directory.join(format!("{label}-stdout.jsonl")))?;
        let stderr = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join(format!("{label}-stderr.log")))?;
        let mut child = Command::new(executable)
            .args(values)
            .creation_flags(CREATE_NO_WINDOW.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(stderr))
            .spawn()?;
        let pipe = child
            .stdout
            .take()
            .ok_or_else(|| Error::other("missing child stdout"))?;
        let (ready, receipt) = mpsc::sync_channel(1);
        let reader = thread::spawn(move || {
            let mut log = stdout_log;
            for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                let _ = writeln!(log, "{line}");
                if serde_json::from_str::<Value>(&line)
                    .ok()
                    .is_some_and(|value| value["kind"] == "ready")
                {
                    let _ = ready.try_send(());
                }
            }
            let _ = log.flush();
        });
        let mut owned = Self {
            child,
            reader: Some(reader),
        };
        if receipt.recv_timeout(Duration::from_secs(30)).is_err() {
            let _ = owned.finish();
            return Err(Error::other(format!("{label} did not initialize")));
        }
        Ok(owned)
    }
    fn finish(&mut self) -> io::Result<Value> {
        let id = self.child.id();
        if let Some(mut input) = self.child.stdin.take() {
            let _ = input.write_all(b"stop\n");
        }
        let started = Instant::now();
        let mut forced = false;
        let status = loop {
            if let Some(status) = self.child.try_wait()? {
                break status;
            }
            if started.elapsed() >= Duration::from_secs(5) {
                forced = true;
                self.child.kill()?;
                break self.child.wait()?;
            }
            thread::sleep(Duration::from_millis(5));
        };
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        Ok(
            json!({"pid":id,"exit_code":status.code(),"forced":forced,"success":status.success()&&!forced,
            "stop_ns":started.elapsed().as_nanos().to_string()}),
        )
    }
}

impl Drop for OwnedRole {
    fn drop(&mut self) {
        if self.reader.is_some() {
            let _ = self.finish();
        }
    }
}

pub fn renderer(mut args: Arguments) -> io::Result<()> {
    let config = RunConfig::read(&mut args)?;
    args.done()?;
    fs::create_dir_all(&config.directory)?;
    if fs::read_dir(&config.directory)?.next().is_some() {
        return Err(Error::other("capture directory must be empty; preserve prior evidence"));
    }
    let adapter = selected(config.adapter_index)?;
    let memory = query_adapter_memory(adapter).map_err(Error::other)?;
    let memory_ceiling = memory
        .admission_limit(config.workload.ceiling())
        .map_err(Error::other)?;
    let executable = std::env::current_exe()?;
    let id = Uuid::new_v4();
    let source_names = [format!("GoldenQ-{id}-A"), format!("GoldenQ-{id}-B")];
    let output_name = format!("GoldenQ-{id}-Output");
    let (width, height) = config.workload.extent();
    let maximum = config.total() + Duration::from_secs(60);
    let mut children = Vec::new();
    for (index, (role, name, w, h)) in [
        ("source", &source_names[0], width, height),
        ("source", &source_names[1], 1280, 720),
        ("consumer", &output_name, width, height),
    ]
    .into_iter()
    .enumerate()
    {
        let child_args = vec![
            "--role".into(),
            role.into(),
            "--adapter".into(),
            config.adapter_index.to_string(),
            "--directory".into(),
            config.directory.to_string_lossy().into_owned(),
            "--name".into(),
            name.clone(),
            "--width".into(),
            w.to_string(),
            "--height".into(),
            h.to_string(),
            "--instance".into(),
            Uuid::new_v4().to_string(),
            "--source-id".into(),
            (index + 1).to_string(),
            "--max-seconds".into(),
            maximum.as_secs().to_string(),
            "--trace-capacity".into(),
            config.trace_capacity.to_string(),
        ];
        children.push(OwnedRole::spawn(
            &executable,
            &child_args,
            &config.directory,
            &format!("fixture-{index}"),
        )?);
    }
    let pids = children.iter().map(|child| child.child.id()).collect::<Vec<_>>();
    let mut manifest = writer(&config.directory.join("manifest.json"))?;
    serde_json::to_writer_pretty(&mut manifest,&json!({"schema_version":golden_video::qualification::protocol::CAPTURE_SCHEMA_VERSION,"run_id":id,"scenario":config.workload,
        "pid":std::process::id(),"fixture_pids":pids,"adapter":adapter,"initial_memory":memory,"memory_ceiling":memory_ceiling,
        "warmup_seconds":config.warmup.as_secs(),"sample_seconds":config.duration.as_secs(),"independence_seconds":config.independence.as_secs(),
        "trace_capacity":config.trace_capacity,"source_names":source_names,"output_name":output_name,
        "debug_assertions":cfg!(debug_assertions),"arch":std::env::consts::ARCH,"qpc_frequency":frequency()?})).map_err(Error::other)?;
    manifest.flush()?;
    let result = host::run(HostConfig {
        scenario: config.workload,
        adapter,
        source_names,
        output_name,
        directory: config.directory.clone(),
        warmup: config.warmup,
        duration: config.duration,
        independence_duration: config.independence,
        memory_ceiling,
        trace_capacity: config.trace_capacity,
        fixture_pids: pids,
    });
    let shutdown = Instant::now();
    let mut exits = Vec::new();
    for child in &mut children {
        exits.push(child.finish()?);
    }
    let mut summary = writer(&config.directory.join("parent-summary.json"))?;
    serde_json::to_writer_pretty(
        &mut summary,
        &json!({"schema_version":golden_video::qualification::protocol::CAPTURE_SCHEMA_VERSION,"renderer_returned_ok":result.is_ok(),
        "children":exits,"children_stop_ns":shutdown.elapsed().as_nanos().to_string(),"qpc":qpc()?}),
    )
    .map_err(Error::other)?;
    summary.flush()?;
    result?;
    if exits.iter().any(|exit| exit["success"] != true) {
        return Err(Error::other("one or more owned fixtures did not retire successfully"));
    }
    Ok(())
}
