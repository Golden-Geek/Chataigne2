use uuid::Uuid;

use super::*;
use crate::runtime::{CompletionToken, DeviceGeneration};

mod hardware;

#[test]
fn final_gpu_coverage_never_retires_an_incomplete_or_foreign_device_token() {
    let device = DeviceGeneration::new(Uuid::new_v4(), 1);
    let token = CompletionToken { device, serial: 10 };
    let mut pending = Some(token);
    assert_eq!(
        gpu::take_completed(
            &mut pending,
            CompletionToken {
                device: device.next().unwrap(),
                serial: 100
            }
        ),
        None
    );
    assert_eq!(
        gpu::take_completed(&mut pending, CompletionToken { device, serial: 9 }),
        None
    );
    assert_eq!(pending, Some(token));
    assert_eq!(
        gpu::take_completed(&mut pending, CompletionToken { device, serial: 10 }),
        Some(token)
    );
    assert_eq!(
        gpu::take_completed(&mut pending, CompletionToken { device, serial: 11 }),
        None,
        "coverage closes once"
    );
}

#[test]
fn gpu_validity_and_offline_rows_preserve_missing_disjoint_and_exact_context_identity() {
    use windows::Win32::Foundation::{S_FALSE, S_OK};
    use windows::core::HRESULT;
    assert_eq!(
        gpu::available(S_FALSE, 0_u64).unwrap(),
        None,
        "not-ready must not fabricate a zero sample"
    );
    assert_eq!(
        gpu::available(S_OK, 0_u64).unwrap(),
        Some(0),
        "an available zero sample is valid"
    );
    assert!(gpu::available(HRESULT(0x80004005_u32 as i32), 0_u64).is_err());
    assert_eq!(
        gpu::validity(0, 0, 1, false),
        GpuSampleStatus::Valid,
        "zero is a valid available timestamp"
    );
    assert_eq!(gpu::validity(8, 7, 1, false), GpuSampleStatus::Invalid);
    assert_eq!(gpu::validity(1, 2, 0, false), GpuSampleStatus::Invalid);
    assert_eq!(gpu::validity(1, 2, 1, true), GpuSampleStatus::Disjoint);
    let token = CompletionToken {
        device: DeviceGeneration::new(Uuid::new_v4(), 2),
        serial: 4,
    };
    let row = DiagnosticEvent::Gpu(GpuTrace {
        token,
        context: DiagnosticContext::FixtureProducer { id: Uuid::nil() },
        status: GpuSampleStatus::Failed { hresult: -1 },
        begin: None,
        end: None,
        frequency: None,
    });
    let decoded: DiagnosticEvent = serde_json::from_str(&serde_json::to_string(&row).unwrap()).unwrap();
    let DiagnosticEvent::Gpu(decoded) = decoded else {
        panic!("wrong telemetry row");
    };
    assert_eq!(decoded.token, token);
    assert_eq!(decoded.context, DiagnosticContext::FixtureProducer { id: Uuid::nil() });
    assert_eq!(decoded.status, GpuSampleStatus::Failed { hresult: -1 });
    assert_eq!((decoded.begin, decoded.end, decoded.frequency), (None, None, None));
}
