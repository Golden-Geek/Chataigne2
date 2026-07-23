use golden_audio::{
    AudioCommand, AudioConfiguration, AudioEngineBuilder, ConfigGeneration, FrameCount, OfflineClock, SampleRate,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = AudioEngineBuilder::default().build()?;
    let events = engine
        .take_event_receiver()
        .ok_or("audio event receiver was already taken")?;
    engine.control().submit(AudioCommand::ApplyConfiguration {
        generation: ConfigGeneration::new(1),
        config: Box::new(AudioConfiguration::empty()),
    })?;

    let mut clock = OfflineClock::new(SampleRate::new(48_000)?)?;
    clock.advance(FrameCount::new(128)?)?;
    println!(
        "offline frame={} elapsed={:?} first_event={:?}",
        clock.frame(),
        clock.elapsed(),
        events.recv()?
    );

    engine.shutdown()?;
    Ok(())
}
