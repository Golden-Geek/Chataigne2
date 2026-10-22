use golden_core::{edit::NodeReadGuard, node::NodeId, process_ctx::ProcessTreeSnapshot};

use super::super::SoundCardValueBindings;

const MAX_TARGETS: usize = 131_072;
const MAX_PATH: usize = 32;
const MAX_BYTES: usize = 32 * 1024 * 1024;

/// Native-prepared path reads. Scalar publication borrows this fixed index and
/// copies at most 32 guards, without acquiring or rebuilding a tree snapshot.
#[derive(Debug, Default)]
pub(crate) struct ProjectionReads {
    targets: Box<[Target]>,
    bytes: usize,
}

#[derive(Debug)]
struct Target {
    node: NodeId,
    path: Box<[NodeReadGuard]>,
}

impl ProjectionReads {
    pub(crate) fn prepare(
        snapshot: &ProcessTreeSnapshot,
        module: NodeId,
        bindings: &SoundCardValueBindings,
    ) -> Result<Self, &'static str> {
        let count = bindings.runtime_values.len();
        if count > MAX_TARGETS {
            return Err("Sound Card projection exceeds 131072 targets");
        }
        let mut bytes = count * size_of::<Target>();
        let mut targets = Vec::with_capacity(count);
        for node in &bindings.runtime_values {
            let mut path = Vec::new();
            let mut current = *node;
            loop {
                if path.len() == MAX_PATH {
                    return Err("Sound Card projection path exceeds 32 nodes");
                }
                let guard = NodeReadGuard::capture_structure(snapshot, current)
                    .ok_or("Sound Card projection target disappeared")?;
                bytes += size_of::<NodeReadGuard>();
                if bytes > MAX_BYTES {
                    return Err("Sound Card projection reads exceed 32 MiB");
                }
                path.push(guard);
                if current == module {
                    break;
                }
                current = snapshot
                    .node(current)
                    .and_then(|node| node.parent)
                    .ok_or("Sound Card projection target is outside its module")?;
            }
            targets.push(Target {
                node: *node,
                path: path.into_boxed_slice(),
            });
        }
        targets.sort_unstable_by_key(|target| target.node.0);
        Ok(Self {
            targets: targets.into_boxed_slice(),
            bytes,
        })
    }

    pub(super) fn path(&self, node: NodeId) -> Option<&[NodeReadGuard]> {
        self.targets
            .binary_search_by_key(&node.0, |target| target.node.0)
            .ok()
            .map(|index| self.targets[index].path.as_ref())
    }

    pub(crate) fn retained_heap_bytes(&self) -> usize {
        self.bytes
    }
}
