import { VersionedNodeMap } from '../graph-index';
import type { NodeId, ParamValue, UiAck, EventTime } from '../../types';
import { compareEventTime } from './event-time';

interface ParameterDraft {
	readonly node: NodeId;
	readonly value: ParamValue;
	readonly enabled?: boolean;
}
export interface ParameterPreview {
	readonly uuid: string;
	readonly value: ParamValue;
	readonly enabled?: boolean;
	readonly current: () => boolean;
}
export interface ParameterPreviews {
	readonly values: VersionedNodeMap<ParameterPreview>;
	get(id: NodeId, uuid: string): ParamValue | undefined;
	enabled(id: NodeId, uuid: string): boolean | undefined;
}
interface EditWork {
	readonly accepted?: boolean;
	finish(after?: () => Promise<void>): Promise<void>;
	cancel(): Promise<void>;
}
export interface EditOperationContext {
	applied(acknowledgements: readonly UiAck[]): void;
	readonly ready: Promise<void>;
	readonly current: boolean;
	publish(edits: readonly ParameterDraft[]): void;
}
export interface EditOperation<Work extends EditWork> extends EditOperationContext {
	readonly work: Work;
	readonly confirmed: boolean;
	confirm(after?: () => Promise<void>): Promise<void>;
	cancel(): Promise<void>;
	detach(): void;
}

/** Session-owned admission, confirmed work and parameter drafts; never an authored graph. */
export function createEditOperations(context: {
	uuid(id: NodeId): string | undefined;
	changed(): void;
	track(): void;
	limit?: number;
	eventTime(): EventTime | undefined;
}) {
	let generation = 0,
		sequence = 0;
	let tail: Promise<void> = Promise.resolve();
	let values = new VersionedNodeMap<ParameterPreview>();
	const records = new Set<{ cancel(): Promise<void>; observe(): void }>();
	const layers = new Map<NodeId, Map<number, ParameterPreview>>();
	const latest = (id: NodeId, uuid: string) => {
		context.track();
		const entry = values.get(id);
		if (entry?.uuid === uuid && entry.current()) return entry;
		let result: ParameterPreview | undefined;
		for (const candidate of layers.get(id)?.values() ?? [])
			if (candidate.uuid === uuid && candidate.current()) result = candidate;
		return result;
	};
	const refresh = (ids: Iterable<NodeId>) => {
		const next = values.fork();
		for (const id of ids) {
			let latest: ParameterPreview | undefined;
			for (const entry of layers.get(id)?.values() ?? []) if (entry.current()) latest = entry;
			if (latest) next.set(id, latest);
			else next.delete(id);
		}
		values = next;
		context.changed();
	};
	const open = <Work extends EditWork>(options: {
		current(): boolean;
		build(scope: EditOperationContext): Work;
	}): EditOperation<Work> | null => {
		if (records.size >= (context.limit ?? 64)) return null;
		const id = ++sequence,
			epoch = generation,
			ready = tail;
		let release!: () => void;
		const released = new Promise<void>((resolve) => {
			release = resolve;
		});
		tail = ready.then(() => released);
		let live = true,
			cancelled = false,
			confirmed = false;
		let completed = false,
			succeeded = false,
			boundary: EventTime | undefined;
		let completion: Promise<void> | null = null;
		const projected = new Set<NodeId>();
		const current = () => live && !cancelled && epoch === generation && options.current();
		const clearProjection = () => {
			if (!projected.size) return;
			for (const node of projected) {
				const entries = layers.get(node);
				entries?.delete(id);
				if (entries?.size === 0) layers.delete(node);
			}
			refresh(projected);
			projected.clear();
		};
		const cleanup = () => {
			if (!live) return;
			live = false;
			clearProjection();
			records.delete(record);
			release();
			if (records.size === 0) tail = Promise.resolve();
			context.changed();
		};
		const settle = (done: Promise<void>) => {
			if (!completion) {
				completion = done;
				void done.then(
					() => complete(true),
					() => complete(false)
				);
			}
			return completion;
		};
		const waitingForProjection = () => {
			const at = context.eventTime();
			return (
				confirmed &&
				succeeded &&
				work.accepted !== false &&
				boundary !== undefined &&
				current() &&
				(at === undefined || compareEventTime(at, boundary) < 0)
			);
		};
		const complete = (success: boolean) => {
			completed = true;
			succeeded = success;
			release();
			if (!waitingForProjection()) cleanup();
			else context.changed();
		};
		const operation: EditOperation<Work> = {
			ready,
			get current() {
				return current();
			},
			get confirmed() {
				return confirmed;
			},
			get work() {
				return work;
			},
			applied(acks) {
				for (const ack of acks) {
					const time = ack.latest_event_time ?? ack.earliest_event_time ?? undefined;
					if (
						ack.success &&
						time &&
						(boundary === undefined || compareEventTime(time, boundary) > 0)
					)
						boundary = time;
				}
			},
			publish(edits) {
				if (!current()) return;
				const dirty = new Set(projected);
				for (const node of projected) layers.get(node)?.delete(id);
				projected.clear();
				for (const edit of edits) {
					const uuid = context.uuid(edit.node);
					if (!uuid) continue;
					let entries = layers.get(edit.node);
					if (!entries) layers.set(edit.node, (entries = new Map()));
					entries.set(id, { uuid, value: edit.value, enabled: edit.enabled, current });
					projected.add(edit.node);
					dirty.add(edit.node);
				}
				for (const node of dirty) if (layers.get(node)?.size === 0) layers.delete(node);
				refresh(dirty);
			},
			confirm(after) {
				if (completion) return completion;
				if (!current()) return operation.cancel();
				confirmed = true;
				return settle(work.finish(after));
			},
			cancel() {
				if (cancelled) return completion ?? Promise.resolve();
				cancelled = true;
				clearProjection();
				if (completed) {
					cleanup();
					return completion ?? Promise.resolve();
				}
				return settle(work.cancel());
			},
			detach() {
				if (!confirmed) void operation.cancel();
			}
		};
		const record = {
			cancel: () => operation.cancel(),
			observe: () => {
				if (!current()) void operation.cancel();
				else if (completed && !waitingForProjection()) cleanup();
			}
		};
		records.add(record);
		let work: Work;
		try {
			work = options.build(operation);
		} catch (error) {
			cleanup();
			throw error;
		}
		context.changed();
		return operation;
	};
	return {
		open,
		get size() {
			context.track();
			return records.size;
		},
		get values() {
			context.track();
			return values;
		},
		get(id: NodeId, uuid: string): ParamValue | undefined {
			return latest(id, uuid)?.value;
		},
		enabled(id: NodeId, uuid: string): boolean | undefined {
			return latest(id, uuid)?.enabled;
		},
		reset() {
			generation++;
			for (const record of records) void record.cancel();
		},
		observe() {
			for (const record of records) record.observe();
		}
	};
}
export type UiEditOperations = ReturnType<typeof createEditOperations>;
