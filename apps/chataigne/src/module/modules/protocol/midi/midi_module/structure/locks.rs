use super::*;
use golden_core::edit::{EditCompletion, EditCompletionStatus, EditPreconditions, EditQueue, NodeReadGuard};
use golden_core::node::NodeUuid;

const WARNING: &str = "midi_value_structure";
#[derive(Clone, Copy)]
enum Shape {
    Root,
    Channel,
    Notes,
    Poly,
    Cc,
    CcValue,
    Mtc,
    Clock,
    NoteInfo,
    System,
    Sysex,
    Leaf,
}
struct Frame {
    id: NodeId,
    uuid: NodeUuid,
    parent: Option<NodeId>,
    shape: Shape,
    child: usize,
    lock: bool,
}
pub(super) struct LockWork {
    root: NodeId,
    owner: NodeId,
    uuid: NodeUuid,
    frames: Vec<Frame>,
    receipt: Option<EditCompletion>,
}
impl LockWork {
    pub fn new(snapshot: &ProcessTreeSnapshot, root: NodeId, owner: NodeId) -> Option<Self> {
        let frame = frame(snapshot, root, Shape::Root)?;
        Some(Self {
            root,
            owner,
            uuid: frame.uuid,
            frames: vec![frame],
            receipt: None,
        })
    }
    pub fn requires_snapshot(&self) -> bool {
        self.receipt
            .as_ref()
            .is_none_or(|r| r.status() != EditCompletionStatus::Pending)
    }
    pub fn service(&mut self, ctx: &mut ProcessCtx, snapshot: Option<&ProcessTreeSnapshot>) -> bool {
        if self
            .receipt
            .as_ref()
            .is_some_and(|r| r.status() == EditCompletionStatus::Pending)
        {
            return false;
        }
        let Some(snapshot) = snapshot else {
            return false;
        };
        let retry = match self.receipt.take().map(|r| r.status()) {
            Some(EditCompletionStatus::Pending) => unreachable!("pending receipt retained below"),
            Some(EditCompletionStatus::Failed) => true,
            _ => false,
        };
        let Some(root) = snapshot.node(self.root) else {
            return true;
        };
        if root.uuid != self.uuid || root.parent != Some(self.owner) {
            return true;
        }
        if retry
            || self.frames.iter().any(|f| {
                snapshot
                    .node(f.id)
                    .is_none_or(|n| n.uuid != f.uuid || n.parent != f.parent)
            })
        {
            self.frames = vec![frame(snapshot, self.root, Shape::Root).expect("checked root")];
        }
        if retry {
            ctx.set_node_warning_with(
                self.owner,
                Some(WARNING),
                "MIDI value locking was rejected; rechecking owned surface",
                None,
            );
        }
        let mut guards = Vec::with_capacity(32);
        guards.extend(
            self.frames
                .iter()
                .filter_map(|f| NodeReadGuard::capture(snapshot, f.id)),
        );
        let mut patches = EditQueue::new();
        for _ in 0..64 {
            let Some(top) = self.frames.last_mut() else {
                break;
            };
            if top.lock {
                if patches.pending.len() == 24 {
                    break;
                }
                top.lock = false;
                guards.push(NodeReadGuard::capture(snapshot, top.id).expect("validated lock target"));
                lock_midi_value_node_in(&mut patches, top.id);
                continue;
            }
            let Some((key, shape)) = child(top.shape, top.child) else {
                self.frames.pop();
                continue;
            };
            top.child += 1;
            if let Some(id) = snapshot.find_child_by_decl_id(top.id, &key) {
                if let Some(next) = frame(snapshot, id, shape) {
                    self.frames.push(next);
                }
            }
        }
        assert!(guards.len() <= 32 && self.frames.len() <= 5, "fixed MIDI surface shape");
        if !patches.pending.is_empty() {
            let mut edits = EditQueue::new();
            edits.push(Edit::CheckPreconditions {
                preconditions: EditPreconditions::new(guards, vec![]).expect("bounded MIDI surface reads"),
            });
            edits.pending.extend(patches.drain());
            self.receipt = Some(edits.completion());
            ctx.edits.pending.extend(edits.drain());
            false
        } else if self.frames.is_empty() {
            ctx.clear_node_warning(self.owner, Some(WARNING));
            true
        } else {
            false
        }
    }
}
fn frame(snapshot: &ProcessTreeSnapshot, id: NodeId, shape: Shape) -> Option<Frame> {
    let node = snapshot.node(id)?;
    Some(Frame {
        id,
        uuid: node.uuid,
        parent: node.parent,
        shape,
        child: 0,
        lock: true,
    })
}
fn lock_midi_value_node_in(edits: &mut EditQueue, id: NodeId) {
    edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            user_permissions: Some(NodeUserPermissions::none()),
            ..Default::default()
        },
    });
}
fn child(shape: Shape, index: usize) -> Option<(String, Shape)> {
    use Shape::*;
    let (name, next) = match shape {
        Root => match index {
            0 => ("mtc", Mtc),
            1 => ("midi_clock", Clock),
            2 => ("note_info", NoteInfo),
            3 => (SYSTEM_FOLDER_DECL_ID, System),
            4..=19 => return Some((channel_decl_id((index - 3) as u8), Channel)),
            _ => return None,
        },
        Channel => match index {
            0 => (NOTES_FOLDER_DECL_ID, Notes),
            1 => (CONTROL_CHANGE_FOLDER_DECL_ID, Cc),
            2 => (POLY_PRESSURE_FOLDER_DECL_ID, Poly),
            3 => (PROGRAM_DECL_ID, Leaf),
            4 => (CHANNEL_PRESSURE_DECL_ID, Leaf),
            5 => (PITCH_BEND_DECL_ID, Leaf),
            _ => return None,
        },
        Notes | Poly if index < 128 => return Some((note_decl_id(index as u8), Leaf)),
        Cc if index < 128 => return Some((cc_decl_id(index as u8), CcValue)),
        CcValue if index == 0 => (MIDI_CC_ROTARY_MODE_DECL_ID, Leaf),
        Mtc => (*(["rate", "mtc_playing", "time", "timecode"].get(index)?), Leaf),
        Clock => (
            *([
                "beat",
                START_DECL_ID,
                CONTINUE_DECL_ID,
                STOP_DECL_ID,
                RESET_DECL_ID,
                "playing",
                "bpm",
            ]
            .get(index)?),
            Leaf,
        ),
        NoteInfo => (
            *([
                "note_played",
                "current_note_on",
                "last_channel",
                "last_pitch",
                "last_velocity",
            ]
            .get(index)?),
            Leaf,
        ),
        System => match index {
            0..=9 => (
                *([
                    TIME_CODE_QUARTER_FRAME_DECL_ID,
                    SONG_POSITION_DECL_ID,
                    SONG_SELECT_DECL_ID,
                    TUNE_REQUEST_DECL_ID,
                    TIMING_CLOCK_DECL_ID,
                    START_DECL_ID,
                    CONTINUE_DECL_ID,
                    STOP_DECL_ID,
                    ACTIVE_SENSING_DECL_ID,
                    RESET_DECL_ID,
                ]
                .get(index)?),
                Leaf,
            ),
            10 => (SYSEX_FOLDER_DECL_ID, Sysex),
            _ => return None,
        },
        Sysex => (*([SYSEX_BYTES_DECL_ID, SYSEX_LENGTH_DECL_ID].get(index)?), Leaf),
        _ => return None,
    };
    Some((name.into(), next))
}
