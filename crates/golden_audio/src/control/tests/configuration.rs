use std::sync::Arc;

use crate::{
    AudioBackend, AudioDeviceInventory, AudioEngineBuilder, AudioError, AudioStream, BackendDescriptor, BackendId,
    BackendPolicy, NullBackend, StreamRequest,
};

use super::super::configuration::validate_backends;

#[derive(Debug)]
struct IdentityOnlyBackend;

impl AudioBackend for IdentityOnlyBackend {
    fn id(&self) -> BackendId {
        BackendId::from_static("identity-only")
    }

    fn descriptor(&self) -> BackendDescriptor {
        panic!("backend identity access must not probe the descriptor")
    }

    fn device_inventory(&self) -> Result<AudioDeviceInventory, AudioError> {
        panic!("backend validation must not discover devices")
    }

    fn open_stream(&self, request: &StreamRequest) -> Result<Box<dyn AudioStream>, AudioError> {
        NullBackend.open_stream(request)
    }
}

#[test]
fn backend_registration_and_validation_use_side_effect_free_identity() {
    let backend = Arc::new(IdentityOnlyBackend) as Arc<dyn AudioBackend>;
    let policy = BackendPolicy {
        preferred: vec![BackendId::from_static("identity-only")],
        allow_null_fallback: false,
    };

    validate_backends(&[backend], &policy).expect("identity-only backend should validate");
    let _builder = AudioEngineBuilder::default()
        .without_backends()
        .with_backend(IdentityOnlyBackend);
}
