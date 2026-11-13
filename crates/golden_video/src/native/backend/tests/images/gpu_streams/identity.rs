use super::*;
use crate::image::ImageData;
use crate::native::{DiagnosticEvent, DiagnosticsConfig};
use std::{num::NonZeroUsize, sync::Arc};

#[test]
#[ignore = "explicit Intel source publication/upload identity and retained pixel regression"]
fn hardware_intel_gpu_publication_identity_is_independent_of_upload_version() {
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| adapter.vendor_id == 0x8086 && !adapter.software)
        .unwrap();
    let desc = descriptor(2, ColorEncoding::Linear, AlphaMode::Straight);
    let fixture = prepared_image(desc, &[0; 8]);
    let config = Some(DiagnosticsConfig {
        capacity: NonZeroUsize::new(1024).unwrap(),
    });
    let mut first = Renderer::with_diagnostics(adapter.id, &fixture, 2, config);
    let mut second = Renderer::with_diagnostics(adapter.id, &fixture, 2, config);
    let budget = GpuFrameBudget::new(24);
    let mut owner = GpuFramePublisher::new_software(desc, adapter.id, &budget, None).unwrap();
    let stream = owner.stream();
    // A withdrawal consumes a publication version without performing an upload.
    stream.publish(None).unwrap();
    install(&mut first, stream.clone());
    install(&mut second, stream.clone());
    let image = Arc::new(ImageData::new(desc, vec![127, 127, 127, 255, 127, 127, 127, 255]).unwrap());
    assert!(owner.submit_software(image).unwrap());
    let image = completed(&mut owner);
    let upload = image.software_upload.unwrap();
    assert_eq!(upload.stream, stream.id());
    assert_eq!(upload.sequence, 1);
    stream.publish(Some(image)).unwrap();
    for renderer in [&mut first, &mut second] {
        for _ in 0..2 {
            assert_eq!(renderer.pixel_frame(), vec![[127, 127, 127, 255]; 2]);
        }
        let mut events = Vec::new();
        renderer
            .backend
            .control
            .drain_diagnostics(&mut events, NonZeroUsize::new(1024).unwrap());
        let uses: Vec<_> = events
            .into_iter()
            .filter_map(|event| match event {
                DiagnosticEvent::SourceUse(row) => Some(row),
                _ => None,
            })
            .collect();
        assert_eq!(uses.len(), 2);
        for row in uses {
            assert_eq!(row.stream, Some(stream.id()));
            assert_eq!(row.sequence, 2, "actual retained publication, not upload version1");
            assert_eq!(row.software_upload.unwrap(), upload);
        }
    }
    stream.publish(None).unwrap();
    for renderer in [&mut first, &mut second] {
        assert!(renderer.pixel_frame().iter().all(|pixel| *pixel == [0; 4]));
    }
    drop((first, second, owner, stream));
    assert_eq!(budget.retained_bytes(), 0);
}
