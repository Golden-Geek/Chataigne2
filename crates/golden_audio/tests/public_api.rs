use golden_audio::{
    AudioBackend, AudioCommand, AudioConfiguration, AudioEngineBuilder, AudioEvent, ConfigGeneration, EngineLimits,
    FrameCount, NullBackend, OfflineClock, SampleRate,
};

#[test]
fn external_consumer_can_use_backend_neutral_surface() {
    let backend = NullBackend;
    assert!(!backend.discover().unwrap().is_empty());

    let mut builder = AudioEngineBuilder::default();
    builder.limits = EngineLimits::default();
    let mut engine = builder.build().unwrap();
    let control = engine.control();
    let events = engine.take_event_receiver().unwrap();
    control
        .submit(AudioCommand::ApplyConfiguration {
            generation: ConfigGeneration::new(1),
            config: Box::new(AudioConfiguration::empty()),
        })
        .unwrap();

    loop {
        if matches!(events.recv().unwrap(), AudioEvent::ConfigurationApplied { .. }) {
            break;
        }
    }

    let mut clock = OfflineClock::new(SampleRate::new(48_000).unwrap()).unwrap();
    assert_eq!(clock.advance(FrameCount::new(128).unwrap()).unwrap(), 128);
    engine.shutdown().unwrap();
}
