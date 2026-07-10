use std::{env, path::Path};

use golden_codegen::{CodegenMode, GeneratedArtifact, apply_artifacts};

fn main() {
    let mode = match env::args().nth(1).as_deref() {
        Some("--check") => CodegenMode::Check,
        Some("--print") => {
            print!("{}", golden_protocol::typescript_declarations());
            return;
        }
        Some(argument) => panic!("unsupported argument: {argument}"),
        None => CodegenMode::Write,
    };
    apply_artifacts(
        Path::new("."),
        [GeneratedArtifact {
            relative_path: "packages/golden-runtime-client/src/generated/protocol.ts".into(),
            contents: golden_protocol::typescript_declarations(),
        }],
        mode,
    )
    .expect("protocol generation failed");
}
