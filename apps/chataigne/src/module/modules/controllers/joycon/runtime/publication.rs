use super::super::*;
use super::*;
use crate::app::module::script_api::{NativeCallbackDraft, NativeCallbackPayload, NativeCallbackPreparer};
use golden_core::{node::NodeId, process_ctx::ProcessCtx};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in super::super) struct Settings {
    pub(in super::super) dead_zone: f64,
    pub(in super::super) motion: JoyConMotionDataMode,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            dead_zone: JOYCON_DEFAULT_STICK_DEAD_ZONE,
            motion: JoyConMotionDataMode::None,
        }
    }
}
#[derive(Clone, Copy)]
enum Selector {
    Connection,
    Changed(JoyConButtonParam),
    Edge(JoyConButtonParam),
    Stick,
    Motion,
}
struct Callback {
    selector: Selector,
    payload: NativeCallbackPayload,
}
pub(in super::super) struct Side {
    side: JoyConSide,
    state: JoyConControllerStateSnapshot,
    settings: Settings,
    callbacks: Vec<Callback>,
}
impl Side {
    pub(in super::super) fn emit(
        &self,
        ctx: &mut ProcessCtx,
        module: NodeId,
        previous: &JoyConControllerStateSnapshot,
    ) {
        for callback in &self.callbacks {
            let changed = match callback.selector {
                Selector::Connection => previous.connected != self.state.connected,
                Selector::Changed(button) | Selector::Edge(button) => {
                    joycon_button_state(previous, button) != joycon_button_state(&self.state, button)
                }
                Selector::Stick => {
                    let old = processed_stick_values(previous.stick_x, previous.stick_y, self.settings.dead_zone);
                    let next = processed_stick_values(self.state.stick_x, self.state.stick_y, self.settings.dead_zone);
                    float_changed(old.0, next.0) || float_changed(old.1, next.1)
                }
                Selector::Motion => controller_motion_changed(previous, &self.state, self.settings.motion),
            };
            if changed {
                ctx.emit_custom_event(callback.payload.event(module));
            }
        }
    }
}
enum Row {
    Cached(Arc<Side>),
    Fresh {
        side: JoyConSide,
        state: JoyConControllerStateSnapshot,
        drafts: Vec<(Selector, NativeCallbackDraft)>,
    },
}
pub(super) struct Draft {
    rows: [Row; 2],
    settings: Settings,
}
impl Draft {
    pub(super) fn new(
        state: &JoyConRuntimeState,
        settings: Settings,
        cache: &mut [Option<Arc<Side>>; 2],
    ) -> Result<Self, String> {
        let left = row(JoyConSide::Left, &state.left, settings, &mut cache[0])?;
        let right = row(JoyConSide::Right, &state.right, settings, &mut cache[1])?;
        let draft = Self {
            rows: [left, right],
            settings,
        };
        let mut count = 0;
        let mut bytes = 0;
        for row in &draft.rows {
            if let Row::Fresh { drafts, .. } = row {
                count += drafts.len();
                bytes += drafts.iter().map(|(_, draft)| draft.retained_bytes()).sum::<usize>();
            }
        }
        if count > 50 || bytes > 8 * 1024 * 1024 {
            return Err("Joy-Con callback batch exceeds 50 records / 8 MiB".into());
        }
        Ok(draft)
    }
    pub(super) fn try_finish(&self, preparer: &NativeCallbackPreparer) -> Option<[Arc<Side>; 2]> {
        let mut drafts = Vec::new();
        for row in &self.rows {
            if let Row::Fresh { drafts: entries, .. } = row {
                drafts.extend(entries.iter().map(|(_, draft)| draft));
            }
        }
        let mut payloads = preparer.try_retain_batch(&drafts)?.into_iter();
        Some(std::array::from_fn(|index| match &self.rows[index] {
            Row::Cached(row) => row.clone(),
            Row::Fresh { side, state, drafts } => Arc::new(Side {
                side: *side,
                state: state.clone(),
                settings: self.settings,
                callbacks: drafts
                    .iter()
                    .map(|(selector, _)| Callback {
                        selector: *selector,
                        payload: payloads.next().unwrap(),
                    })
                    .collect(),
            }),
        }))
    }
}
fn row(
    side: JoyConSide,
    state: &JoyConControllerStateSnapshot,
    settings: Settings,
    cache: &mut Option<Arc<Side>>,
) -> Result<Row, String> {
    if let Some(row) = cache
        .as_ref()
        .filter(|row| row.state == *state && row.settings == settings && row.side == side)
    {
        return Ok(Row::Cached(row.clone()));
    }
    // Old unused candidates must not hold quota needed to replace this row.
    *cache = None;
    let mut drafts = Vec::with_capacity(25);
    let snapshot = controller_arg(side, state, settings);
    let name = serde_json::json!(joycon_side_id(side));
    drafts.push((
        Selector::Connection,
        NativeCallbackDraft::new(
            if state.connected {
                JOYCON_CONNECTED_CALLBACK
            } else {
                JOYCON_DISCONNECTED_CALLBACK
            },
            vec![name.clone(), snapshot.clone()],
        )?,
    ));
    if state.connected {
        for button in joycon_side_button_params(side) {
            let pressed = joycon_button_state(state, *button);
            let button_name = serde_json::json!(joycon_button_id(*button));
            drafts.push((
                Selector::Changed(*button),
                NativeCallbackDraft::new(
                    JOYCON_BUTTON_CHANGED_CALLBACK,
                    vec![
                        name.clone(),
                        button_name.clone(),
                        serde_json::json!(pressed),
                        snapshot.clone(),
                    ],
                )?,
            ));
            drafts.push((
                Selector::Edge(*button),
                NativeCallbackDraft::new(
                    if pressed {
                        JOYCON_BUTTON_PRESSED_CALLBACK
                    } else {
                        JOYCON_BUTTON_RELEASED_CALLBACK
                    },
                    vec![name.clone(), button_name, snapshot.clone()],
                )?,
            ));
        }
        let stick = processed_stick_values(state.stick_x, state.stick_y, settings.dead_zone);
        drafts.push((
            Selector::Stick,
            NativeCallbackDraft::new(
                JOYCON_STICK_CHANGED_CALLBACK,
                vec![
                    name.clone(),
                    serde_json::json!({"x":stick.0,"y":stick.1}),
                    snapshot.clone(),
                ],
            )?,
        ));
        if settings.motion != JoyConMotionDataMode::None {
            drafts.push((
                Selector::Motion,
                NativeCallbackDraft::new(
                    JOYCON_MOTION_CHANGED_CALLBACK,
                    vec![name, controller_motion_arg(state, settings.motion), snapshot],
                )?,
            ));
        }
    }
    Ok(Row::Fresh {
        side,
        state: state.clone(),
        drafts,
    })
}
fn controller_arg(side: JoyConSide, state: &JoyConControllerStateSnapshot, settings: Settings) -> serde_json::Value {
    let stick = processed_stick_values(state.stick_x, state.stick_y, settings.dead_zone);
    serde_json::json!({"side":joycon_side_id(side),"connected":state.connected,"stick":{"x":stick.0,"y":stick.1},"buttons":joycon_buttons_arg(side,state),"motion":controller_motion_arg(state,settings.motion)})
}

pub(super) struct Pending {
    event: JoyConWorkerEvent,
    guards: Vec<RevisionGuard>,
    settings: Option<Settings>,
    callbacks: Option<Draft>,
    reservation: golden_io::PayloadReservation,
}
impl Pending {
    pub(super) fn begin(
        event: JoyConWorkerEvent,
        guards: &[RevisionGuard],
        control: &ProjectionControl,
        pool: &PayloadPool,
        cache: &mut [Option<Arc<Side>>; 2],
        config: &RevisionReader,
    ) -> Result<Self, JoyConWorkerEvent> {
        let reservation = match pool.try_reserve(EVENT_WEIGHT) {
            Ok(value) => value,
            Err(_) => return Err(event),
        };
        let event = normalize(event);
        let mut reads = guards.to_vec();
        let (settings, callbacks) = if let JoyConWorkerEvent::State(state) = &event {
            reads.push(config.guard_at(control.version));
            match Draft::new(state, control.settings, cache) {
                Ok(draft) => (Some(control.settings), Some(draft)),
                Err(error) => {
                    return Ok(Self {
                        event: JoyConWorkerEvent::Error(error),
                        guards: reads,
                        settings: None,
                        callbacks: None,
                        reservation,
                    })
                }
            }
        } else {
            (None, None)
        };
        Ok(Self {
            event,
            guards: reads,
            settings,
            callbacks,
            reservation,
        })
    }
    pub(super) fn is_state(&self) -> bool {
        self.settings.is_some()
    }
    pub(super) fn try_finish(&self, preparer: Option<&NativeCallbackPreparer>) -> Option<Option<[Arc<Side>; 2]>> {
        match &self.callbacks {
            Some(draft) => draft.try_finish(preparer?).map(Some),
            None => Some(None),
        }
    }
    pub(super) fn finish(self, rows: Option<[Arc<Side>; 2]>) -> RetainedPayload<Publication> {
        self.reservation
            .retain(
                Publication {
                    event: self.event,
                    guards: self.guards,
                    rows,
                    settings: self.settings,
                },
                EVENT_WEIGHT,
            )
            .unwrap()
    }
}
pub(super) fn normalize(event: JoyConWorkerEvent) -> JoyConWorkerEvent {
    match event {
        JoyConWorkerEvent::Error(message) => JoyConWorkerEvent::Error(bounded_text(message)),
        JoyConWorkerEvent::State(state)
            if JoyConSide::ALL.iter().any(|side| {
                let value = state.side(*side);
                [
                    value.left_buttons.len(),
                    value.right_buttons.len(),
                    value.shared_buttons.len(),
                ]
                .iter()
                .any(|len| *len > 16)
                    || [
                        value.stick_x,
                        value.stick_y,
                        value.orientation_pitch,
                        value.orientation_roll,
                        value.accelerometer.0,
                        value.accelerometer.1,
                        value.accelerometer.2,
                        value.gyroscope.0,
                        value.gyroscope.1,
                        value.gyroscope.2,
                    ]
                    .iter()
                    .any(|value| !value.is_finite())
            }) =>
        {
            JoyConWorkerEvent::Error("Joy-Con report exceeds its fixed finite state vocabulary".into())
        }
        other => other,
    }
}
