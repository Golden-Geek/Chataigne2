use allocation_counter::measure;

use crate::{PlanarBuffer, RenderProcessor};

use super::support::{compile, one_to_one_fixture};

#[test]
fn warmed_render_allocates_and_deallocates_nothing() {
    let fixture = one_to_one_fixture(32, true);
    let plan = compile(&fixture);
    let mut processor = RenderProcessor::new(plan).unwrap();
    let input = PlanarBuffer::new(32, 511).unwrap();
    let playback = PlanarBuffer::new(32, 511).unwrap();
    let mut output = PlanarBuffer::new(32, 511).unwrap();
    processor.render(&input, &playback, &mut output, 511).unwrap();

    let allocations = measure(|| {
        processor.render(&input, &playback, &mut output, 511).unwrap();
    });
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    assert_eq!(allocations.count_current, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_total, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_current, 0, "{allocations:?}");
}
