import type { NodeId, UiEditIntent } from '../../../../types';
import type { FreeLayoutPreview, FreeLayoutPreviewSet } from './contracts';
import { cloneFreeLayoutPreviewSet, sameCssValue, samePosition } from './geometry';

type Edit = Extract<UiEditIntent, { kind: 'setParam' }>;
interface CommitContext {
	current(): boolean;
	begin(): Promise<void>;
	end(): Promise<void>;
	send(edits: Edit[]): Promise<boolean>;
	frame(callback: () => void): () => void;
	publish?(edits: Edit[]): void;
}

/** One gesture owns its coalesced samples, admitted prefix and acknowledgement baseline. */
export function createFreeLayoutCommitter(initial: FreeLayoutPreviewSet, context: CommitContext) {
	let accepting = true,
		cancelled = false;
	let pending: FreeLayoutPreviewSet | null = null;
	let cancelFrame: (() => void) | null = null;
	let running: Promise<void> | null = null;
	let closing: Promise<void> | null = null;
	const ready = context.begin();
	const baseline = new Map<NodeId, FreeLayoutPreview>(
		cloneFreeLayoutPreviewSet(initial).previews.map(({ target, preview }) => [
			target.nodeId,
			preview
		])
	);
	const last = new Map(baseline),
		touched = new Set<NodeId>();
	let requested = cloneFreeLayoutPreviewSet(initial);
	const valid = () => !cancelled && context.current();
	const editsFor = (
		sample: FreeLayoutPreviewSet,
		comparison = last,
		retained?: Set<NodeId>
	): Edit[] => {
		const edits: Edit[] = [];
		const include = (id: NodeId, changed: boolean) => {
			if (changed) retained?.add(id);
			return changed || !!retained?.has(id);
		};
		for (const { target, preview } of sample.previews) {
			const previous = comparison.get(target.nodeId) ?? target.startPreview;
			if (
				target.positionParamNodeId !== null &&
				target.positionParamBehaviour !== null &&
				include(target.positionParamNodeId, !samePosition(preview.position, previous.position))
			)
				edits.push({
					kind: 'setParam',
					node: target.positionParamNodeId,
					behaviour: target.positionParamBehaviour,
					value: { kind: 'vec2', value: [preview.position.x, preview.position.y] }
				});
			for (const axis of ['width', 'height'] as const) {
				const id = axis === 'width' ? target.widthParamNodeId : target.heightParamNodeId;
				const behaviour =
					axis === 'width' ? target.widthParamBehaviour : target.heightParamBehaviour;
				if (
					id !== null &&
					behaviour !== null &&
					include(id, !sameCssValue(preview.size[axis], previous.size[axis]))
				)
					edits.push({
						kind: 'setParam',
						node: id,
						behaviour,
						value: { kind: 'css_value', ...preview.size[axis] }
					});
			}
		}
		return edits;
	};
	const drain = (): Promise<void> => {
		if (running) return running;
		const task = (async () => {
			await ready;
			while (pending && valid()) {
				const sample = pending;
				pending = null;
				const edits = editsFor(sample);
				const applied = edits.length === 0 || (await context.send(edits));
				if (applied && valid())
					for (const { target, preview } of sample.previews) last.set(target.nodeId, preview);
			}
			if (!valid()) pending = null;
		})();
		running = task;
		const clear = () => {
			if (running === task) running = null;
		};
		void task.then(clear, clear);
		return task;
	};
	const stopFrame = () => {
		cancelFrame?.();
		cancelFrame = null;
	};
	const finish = (after?: () => Promise<void>): Promise<void> => {
		if (closing) return closing;
		accepting = false;
		stopFrame();
		closing = (async () => {
			try {
				await drain();
				if (valid()) await after?.();
			} finally {
				pending = null;
				await context.end();
			}
		})();
		return closing;
	};
	return {
		get accepted() {
			return editsFor(requested).length === 0;
		},
		schedule(sample: FreeLayoutPreviewSet) {
			if (!accepting || !valid()) return;
			pending = cloneFreeLayoutPreviewSet(sample);
			requested = pending;
			context.publish?.(editsFor(pending, baseline, touched));
			if (cancelFrame) return;
			cancelFrame = context.frame(() => {
				cancelFrame = null;
				void drain();
			});
		},
		discardPending() {
			stopFrame();
			pending = null;
		},
		finish,
		cancel() {
			cancelled = true;
			pending = null;
			stopFrame();
			return finish();
		}
	};
}
