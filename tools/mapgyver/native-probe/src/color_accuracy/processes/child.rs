use super::super::{
    DeviceReport,
    protocol::{Report, Request, Response, Role},
    readback, upload,
};
use crate::{ProbeResult, d3d::Gpu, spout::Endpoint, wire};
use mapgyver_native_probe::{
    color_reference::{self, CardInput, Comparison, Fixture, Image},
    workload::Scenario,
};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

type Control = mpsc::Receiver<Result<Option<Request>, String>>;

pub fn run(adapter: u32, scenario: Scenario, fixture: Fixture, role: Role, name: &str) -> ProbeResult<()> {
    if !scenario.cards() {
        return Err("accuracy child requires q01/q02".into());
    }
    let gpu = Gpu::new(adapter)?;
    let (tx, control) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("color-child-control".into())
        .spawn(move || {
            let mut stdin = std::io::stdin().lock();
            loop {
                let request = wire::read_message::<Request>(&mut stdin).map_err(|e| e.to_string());
                let terminal = !matches!(&request, Ok(Some(_)));
                if tx.send(request).is_err() || terminal {
                    break;
                }
            }
        })?;
    let result = match role.channel() {
        Some(channel) => produce(&gpu, scenario, fixture, role, channel, name, &control),
        None => consume(&gpu, scenario, fixture, name, &control),
    };
    let retirement = gpu.retire();
    let (comparison, sender_unregistered, initial_pixel_upload_bytes, full_image_readbacks) = result?;
    wire::respond(&Response::Finished {
        report: Box::new(Report {
            process_id: std::process::id(),
            adapter_luid: gpu.adapter.luid.clone(),
            role,
            fixture,
            initial_pixel_upload_bytes,
            full_image_readbacks,
            sender_unregistered,
            comparison,
            device: DeviceReport {
                role: role.name().into(),
                operations: gpu.allocations.operations(),
                after_fence: gpu.allocations.snapshot(),
                retirement_ms: retirement?,
            },
        }),
    })
}

type Outcome = (Option<Comparison>, Option<bool>, u64, u32);

fn ready(gpu: &Gpu, role: Role, fixture: Fixture) -> ProbeResult<()> {
    wire::respond(&Response::Ready {
        process_id: std::process::id(),
        adapter_luid: gpu.adapter.luid.clone(),
        role,
        fixture,
    })
}

fn request(control: &Control) -> ProbeResult<Option<Request>> {
    match control.try_recv() {
        Ok(Ok(Some(r))) => Ok(Some(r)),
        Ok(Ok(None)) | Err(mpsc::TryRecvError::Disconnected) => Ok(Some(Request::Stop)),
        Ok(Err(e)) => Err(e.into()),
        Err(mpsc::TryRecvError::Empty) => Ok(None),
    }
}

fn produce(
    gpu: &Gpu,
    scenario: Scenario,
    fixture: Fixture,
    role: Role,
    channel: u32,
    name: &str,
    control: &Control,
) -> ProbeResult<Outcome> {
    let extent = if channel == 0 { scenario.extent() } else { [1280, 720] };
    let (texture, bytes) = match fixture {
        Fixture::Authored => {
            let image = Image::card(extent, channel)?;
            let bytes = image.pixels().len() as u64 * 4;
            (upload(gpu, &image)?, bytes)
        }
        Fixture::Procedural => {
            let texture = gpu.texture(extent[0], extent[1])?;
            crate::card::Card::new(gpu)?.draw_input(gpu, &texture, CardInput::fixed(channel)?)?;
            gpu.wait_submitted()?;
            (texture, 0)
        }
    };
    {
        let mut sender = Endpoint::sender(gpu, name)?;
        sender.send(&texture)?;
        gpu.wait_submitted()?;
        if sender.name() != name {
            return Err("accuracy sender was renamed".into());
        }
        ready(gpu, role, fixture)?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match request(control)? {
                Some(Request::Stop) => break,
                Some(Request::Capture) => return Err("producer received capture request".into()),
                None => {}
            }
            if Instant::now() >= deadline {
                return Err("accuracy producer watchdog expired".into());
            }
            sender.send(&texture)?;
            gpu.wait_submitted()?;
            std::thread::sleep(Duration::from_millis(16));
        }
    }
    let registry = Endpoint::new(gpu)?;
    Ok((None, Some(!registry.sender_exists(name)?), bytes, 0))
}

fn consume(gpu: &Gpu, scenario: Scenario, fixture: Fixture, name: &str, control: &Control) -> ProbeResult<Outcome> {
    let mut receiver = Endpoint::receiver(gpu, name)?;
    ready(gpu, Role::Consumer, fixture)?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match request(control)? {
            Some(Request::Capture) => break,
            Some(Request::Stop) => return Err("accuracy consumer cancelled before capture".into()),
            None => {}
        }
        if Instant::now() >= deadline {
            return Err("accuracy consumer capture request timed out".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let pixels = loop {
        if request(control)?.is_some() {
            return Err("accuracy consumer cancelled or received duplicate capture".into());
        }
        if let Some(frame) = receiver.receive()? {
            if [frame.width, frame.height] != scenario.extent() {
                return Err("accuracy consumer extent changed".into());
            }
            break readback::image(gpu, &frame.texture, scenario.extent())?;
        }
        if Instant::now() >= deadline {
            return Err("accuracy consumer input timed out".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    drop(receiver);
    let images = [
        fixture.reference(scenario.extent(), 0)?,
        fixture.reference([1280, 720], 1)?,
    ];
    Ok((
        Some(color_reference::compare(&images, scenario.extent(), &pixels)?),
        None,
        0,
        1,
    ))
}
