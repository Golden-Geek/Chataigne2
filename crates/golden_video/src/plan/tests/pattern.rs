use super::*;

#[test]
fn procedural_layers_have_no_texture_dependencies_but_consume_draw_admission_units() {
    let mut plan = RenderPlan {
        passes: vec![clear(1, 1)],
        outputs: vec![output(1, 1)],
        ..Default::default()
    };
    let pattern = Pattern::Bars([LinearColor::TRANSPARENT; 8]);
    plan.passes[0].operation = RenderOperation::Composite {
        layers: (0..3)
            .map(|_| Layer {
                source: LayerSource::Pattern(pattern),
                opacity: crate::Opacity::OPAQUE,
                blend: BlendMode::Normal,
                filter: SampleFilter::Nearest,
                mapping: None,
            })
            .collect(),
    };
    let mut bounded = limits();
    bounded.max_dependencies = 2;
    assert!(plan.clone().compile(&bounded).is_err());
    bounded.max_dependencies = 3;
    let compiled = plan.compile(&bounded).unwrap();
    assert!(compiled.external_textures().is_empty());
    assert_eq!(compiled.passes().len(), 1);
    assert!(GridPattern::new([0, 1], 0.2, LinearColor::TRANSPARENT, LinearColor::TRANSPARENT).is_none());
    assert!(GridPattern::new([1, 1], f32::NAN, LinearColor::TRANSPARENT, LinearColor::TRANSPARENT).is_none());
    assert!(GridPattern::new([1, 1], 0.5, LinearColor::TRANSPARENT, LinearColor::TRANSPARENT).is_none());
}
