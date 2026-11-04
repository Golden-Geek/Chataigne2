use crate::{LinearColor, plan::*};

fn style() -> CanvasTextStyle {
    CanvasTextStyle {
        cells: [1.; 2],
        glyph: [0.05, 0.1],
        inset: [0.01; 2],
        centered: false,
    }
}
fn primitive(text: CanvasText, style: CanvasTextStyle) -> Option<CanvasPrimitive> {
    CanvasPrimitive::text(
        UnitRect::FULL,
        text,
        style,
        CanvasPaint::solid(LinearColor::TRANSPARENT),
    )
}

#[test]
fn text_rejects_unbounded_control_unicode_and_nonfinite_geometry() {
    for value in ["", "é", "A\n", "\0"] {
        assert!(CanvasText::literal(value).is_none());
    }
    assert!(CanvasText::literal(&"A".repeat(64)).is_some());
    assert!(CanvasText::literal(&"A".repeat(65)).is_none());
    assert!(CanvasText::tiles(&["123456789"]).is_none());
    assert!(CanvasText::tiles(&vec!["A"; 1025]).is_none());
    assert!(
        primitive(
            CanvasText::local_time(),
            CanvasTextStyle {
                glyph: [f32::NAN, 0.1],
                ..style()
            }
        )
        .is_none()
    );
    assert!(
        primitive(
            CanvasText::frame_rate(),
            CanvasTextStyle {
                cells: [2., 1.],
                ..style()
            }
        )
        .is_none()
    );
}

#[test]
fn tiled_text_matches_partial_cells_and_uses_the_canvas_character_cap() {
    let tiled_style = CanvasTextStyle {
        cells: [2.5, 1.],
        ..style()
    };
    assert!(primitive(CanvasText::tiles(&["A1", "B1"]).unwrap(), tiled_style).is_none());
    assert!(primitive(CanvasText::tiles(&["A1", "B1", "C1"]).unwrap(), tiled_style).is_some());
    let text = CanvasText::tiles(&vec!["A"; 1024]).unwrap();
    let command = primitive(
        text,
        CanvasTextStyle {
            cells: [32.; 2],
            ..style()
        },
    )
    .unwrap();
    assert!(CanvasPattern::new(vec![command.clone()]).is_some());
    assert!(CanvasPattern::new(vec![command, primitive(CanvasText::local_time(), style()).unwrap()]).is_none());
}
