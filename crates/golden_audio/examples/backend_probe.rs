use golden_audio::{AudioBackendState, probe_cpal_backends};

fn main() {
    println!("golden_audio CPAL backend probe");
    for backend in probe_cpal_backends() {
        let state = match backend.state {
            AudioBackendState::Compiled => "compiled",
            AudioBackendState::Available => "available",
            AudioBackendState::Unavailable => "unavailable",
            AudioBackendState::MissingServer => "missing-server",
            AudioBackendState::MissingDriver => "missing-driver",
            AudioBackendState::Failed => "failed",
        };
        match backend.detail {
            Some(detail) => println!("{} ({})\t{state}\t{detail}", backend.label, backend.id),
            None => println!("{} ({})\t{state}", backend.label, backend.id),
        }
    }
}
