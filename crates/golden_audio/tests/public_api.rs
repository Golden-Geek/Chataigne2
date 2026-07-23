use golden_audio::{
    AudioBackend, AudioCallbackTimestamp, AudioCommand, AudioConfiguration, AudioEngineBuilder, AudioEvent,
    AudioStreamHandler, ConfigGeneration, EngineLimits, FrameCount, InterleavedOutput, NullBackend, OfflineClock,
    SampleRate,
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

#[derive(Debug)]
struct DefaultOutputHandler;

impl AudioStreamHandler for DefaultOutputHandler {}

#[test]
fn external_handler_default_is_safe_silence() {
    let mut samples = [1.0_f32; 8];
    DefaultOutputHandler.process_output(InterleavedOutput::F32(&mut samples), AudioCallbackTimestamp::default());
    assert_eq!(samples, [0.0; 8]);
}
