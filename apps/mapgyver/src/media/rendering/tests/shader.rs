use super::*;
use golden_video::shader::{FragmentFormat, TEXTURE_INPUTS};

fn configure_texture(engine: &mut AppEngine, reference: ChannelReference, input: Option<ChannelReference>) {
    let mut textures = [None; TEXTURE_INPUTS];
    textures[0] = input.map(|channel| channel.channel.0);
    edit(
        engine,
        MediaEdit::ConfigureChannel {
            reference,
            source: ChannelSource::Shader {
                format: FragmentFormat::Glsl,
                input: ShaderInput::Embedded {
                    code: "void main() { fragColor = texture(iChannel0, vec2(0.5)); }".into(),
                },
                textures,
            },
        },
    );
}

#[test]
fn shader_feedback_clears_the_cyclic_branch_and_recovers_after_rebinding() {
    let mut engine = engine();
    let first = create(&mut engine, MediaKind::Shader);
    let second = create(&mut engine, MediaKind::Shader);
    let independent = create(&mut engine, MediaKind::SolidColor);
    configure_texture(&mut engine, first, Some(second));
    configure_texture(&mut engine, second, Some(first));

    let roots = [first, independent];
    let capture = capture_channels(&engine, &roots, 16).unwrap();
    let (_, rejected) = super::super::derived::ordered(&capture, &roots, 16, 16, 16).unwrap();
    assert_eq!(rejected, std::collections::HashSet::from([first]));
    let reachable = super::super::derived::reachable(&capture, &roots, &rejected);
    assert_eq!(reachable, std::collections::HashSet::from([first, independent]));

    configure_texture(&mut engine, second, Some(independent));
    let capture = capture_channels(&engine, &roots, 16).unwrap();
    let (ordered, rejected) = super::super::derived::ordered(&capture, &roots, 16, 16, 16).unwrap();
    assert!(rejected.is_empty());
    assert_eq!(ordered, [independent, second, first]);
}
