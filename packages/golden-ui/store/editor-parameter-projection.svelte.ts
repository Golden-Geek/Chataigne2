import type { EventTime } from '../types';
import type { AppliedIntentObserver } from './session/intent-sender';
import { compareEventTime } from './session/event-time.ts';
import {
	editorParameterProjection,
	type EditorParameterProjectionHost
} from './editor-parameter-projection';

/** Shared receipt fence for an editor's own writes as well as its read snapshots. */
export function createEditorParameterProjection(host: () => EditorParameterProjectionHost | null) {
	let accepted = $state.raw<{
		host: WeakRef<EditorParameterProjectionHost>;
		generation: number;
		at: EventTime;
	} | null>(null);
	return {
		applied(): AppliedIntentObserver {
			const current = host(),
				generation = current?.snapshotGeneration;
			return (acks) => {
				if (
					!current ||
					host() !== current ||
					current.status !== 'connected' ||
					current.loading.visible ||
					current.snapshotGeneration !== generation
				)
					return;
				for (const ack of acks) {
					const at = ack.latest_event_time ?? ack.earliest_event_time;
					if (
						ack.success &&
						at &&
						(!accepted ||
							accepted.host.deref() !== current ||
							accepted.generation !== generation ||
							compareEventTime(at, accepted.at) > 0)
					)
						// Retained panels must not pin a replaced session and its document graph.
						accepted = { host: new WeakRef(current), generation: current.snapshotGeneration, at };
				}
			};
		},
		read(generation: number | null, at: EventTime) {
			const current = host();
			const boundary =
				accepted?.host.deref() === current &&
				accepted.generation === current?.snapshotGeneration &&
				compareEventTime(accepted.at, at) > 0
					? accepted.at
					: at;
			return editorParameterProjection(current, generation, boundary);
		}
	};
}
