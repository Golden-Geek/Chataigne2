use super::*;
use crate::media::protocol::{ChannelId, MediaId};
use golden_core::node::NodeUuid;
use golden_video::shader::FragmentEffect;

#[test]
fn catalog_keeps_a_revision_for_one_program_and_rejects_old_sources_and_owners() {
    let catalog = Catalog::default();
    let owner = Arc::new(());
    let replacement = Arc::new(());
    let reference = ChannelReference {
        media: MediaId(NodeUuid(uuid::Uuid::new_v4())),
        channel: ChannelId(NodeUuid(uuid::Uuid::new_v4())),
    };
    let file = crate::media::parse_file("effect.glsl").unwrap().unwrap();
    let other = crate::media::parse_file("other.glsl").unwrap().unwrap();
    let compile = |default| {
        FragmentEffect::compile(
            FragmentFormat::Glsl,
            &format!("uniform float gain; //{default}\nvoid main() {{ fragColor = vec4(gain); }}"),
        )
        .unwrap()
    };
    let first = compile("0.25");
    let second = compile("0.75");
    let entry = |effect: &FragmentEffect| PreparedFile {
        reference,
        format: FragmentFormat::Glsl,
        file: file.clone(),
        uniforms: effect.uniform_layout(),
    };
    catalog.publish(&owner, vec![entry(&first)]);
    let first_revision = catalog.revision(reference).unwrap();
    catalog.publish(&owner, vec![entry(&first)]);
    assert_eq!(catalog.revision(reference), Some(first_revision));
    assert!(catalog.get(reference, FragmentFormat::Glsl, &other).is_none());
    assert!(catalog.get(reference, FragmentFormat::ShaderToy, &file).is_none());
    catalog.publish(&owner, vec![entry(&second)]);
    let second_revision = catalog.revision(reference).unwrap();
    assert_ne!(second_revision, first_revision);
    catalog.publish(&replacement, vec![entry(&second)]);
    let revision = catalog.revision(reference).unwrap();
    assert_ne!(revision, second_revision);
    catalog.clear(&owner);
    assert_eq!(catalog.revision(reference), Some(revision));
    drop(replacement);
    assert_eq!(catalog.revision(reference), None);
}
