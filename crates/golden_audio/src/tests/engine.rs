use crate::{AudioCommand, AudioConfiguration, AudioEngineBuilder, AudioErrorCategory, AudioEvent, ConfigGeneration};

#[test]
fn engine_applies_configuration_and_shuts_down_twice() {
    let mut engine = AudioEngineBuilder::default().build().unwrap();
    let events = engine.take_event_receiver().unwrap();
    let control = engine.control();
    let generation = ConfigGeneration::new(1);
    control
        .submit(AudioCommand::ApplyConfiguration {
            generation,
            config: Box::new(AudioConfiguration::empty()),
        })
        .unwrap();

    let mut applied = false;
    while let Ok(event) = events.recv() {
        if event == (AudioEvent::ConfigurationApplied { generation }) {
            applied = true;
            break;
        }
    }
    assert!(applied);
    assert_eq!(engine.observations().latest().generation, generation);

    engine.shutdown().unwrap();
    engine.shutdown().unwrap();
    let error = control
        .submit(AudioCommand::SetEnabled(true))
        .expect_err("stopped engine must reject commands");
    assert_eq!(error.category, AudioErrorCategory::ShuttingDown);
}

#[test]
fn invalid_generation_leaves_last_valid_observation_active() {
    let mut engine = AudioEngineBuilder::default().build().unwrap();
    let events = engine.take_event_receiver().unwrap();
    let control = engine.control();
    let valid_generation = ConfigGeneration::new(4);
    control
        .submit(AudioCommand::ApplyConfiguration {
            generation: valid_generation,
            config: Box::new(AudioConfiguration::empty()),
        })
        .unwrap();

    let invalid_generation = ConfigGeneration::new(5);
    let mut invalid = AudioConfiguration::empty();
    invalid.output.enabled = true;
    control
        .submit(AudioCommand::ApplyConfiguration {
            generation: invalid_generation,
            config: Box::new(invalid),
        })
        .unwrap();

    let mut rejected = false;
    for _ in 0..8 {
        let event = events.recv().unwrap();
        if matches!(
            event,
            AudioEvent::ConfigurationRejected {
                generation,
                ..
            } if generation == invalid_generation
        ) {
            rejected = true;
            break;
        }
    }
    assert!(rejected);
    assert_eq!(engine.observations().latest().generation, valid_generation);
    engine.shutdown().unwrap();
}
