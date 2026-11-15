import { untrack } from 'svelte';
import { createUiEditSession, type UiEditSession } from '../../../store/ui-intents';
import type { WorkbenchSession } from '../../../store/workbench.svelte';
import type { NodeId, UiNodeDto } from '../../../types';
import type { CurveNodes } from './model';
import type { ParamNodeRef, DragPreview, BezierEasingPreview } from './types';
import type { createCurveParameterWriter } from './parameter-writer';
import { curveParametersWritable } from './admission';

interface Scope {
	host: WorkbenchSession;
	project: number | null;
	snapshot: number;
	owner: string;
	selection: string;
	parameters: ParamNodeRef[];
}

/** One admitted edit owns its scope, latest frame samples and pending writes through closure. */
export function createCurveEditGesture(context: {
	readonly session: WorkbenchSession | null;
	readonly nodes: CurveNodes | null;
	readonly owner: UiNodeDto;
	readonly selection: string;
	readonly busy: boolean;
	writer: ReturnType<typeof createCurveParameterWriter>;
	cancel(): void;
}) {
	let scope = $state.raw<Scope | null>(null);
	let edit: UiEditSession | null = null;
	let beginning: Promise<void> | null = null;
	let keys: Map<NodeId, DragPreview> | null = null;
	let bezier: Map<NodeId, BezierEasingPreview> | null = null;
	let frame = 0;
	const current = () => {
		const session = context.session;
		return (
			!!scope &&
			session === scope.host &&
			session.status === 'connected' &&
			!session.loading.visible &&
			session.projectGeneration === scope.project &&
			session.snapshotGeneration === scope.snapshot &&
			context.owner.uuid === scope.owner &&
			context.selection === scope.selection &&
			context.nodes?.get(context.owner.node_id)?.uuid === scope.owner &&
			curveParametersWritable(context.nodes, scope.parameters)
		);
	};
	const discard = () => {
		if (frame !== 0 && typeof window !== 'undefined') window.cancelAnimationFrame(frame);
		frame = 0;
		keys = null;
		bezier = null;
	};
	const flush = () => {
		if (current() && keys?.size) context.writer.keys(keys);
		keys = null;
		if (current() && bezier?.size) context.writer.bezier(bezier);
		bezier = null;
	};
	const schedule = () => {
		if (frame !== 0 || typeof window === 'undefined') return;
		frame = window.requestAnimationFrame(() => {
			frame = 0;
			flush();
		});
	};
	$effect(() => {
		const selected = context.selection;
		untrack(() => {
			if (scope && scope.selection !== selected) context.cancel();
		});
	});
	const accessible = $derived(
		scope === null || curveParametersWritable(context.nodes, scope.parameters)
	);
	$effect(() => {
		if (!accessible) untrack(() => context.cancel());
	});
	return {
		begin(label: string, prefix: string, parameters: ParamNodeRef[]): boolean {
			const session = context.session;
			if (
				!session ||
				session.status !== 'connected' ||
				session.loading.visible ||
				session.hasActiveEditSession ||
				context.busy ||
				session.projectGeneration === null ||
				context.nodes?.get(context.owner.node_id)?.uuid !== context.owner.uuid ||
				!curveParametersWritable(context.nodes, parameters)
			)
				return false;
			scope = {
				host: session,
				project: session.projectGeneration,
				snapshot: session.snapshotGeneration,
				owner: context.owner.uuid,
				selection: context.selection,
				parameters
			};
			edit = createUiEditSession(label, prefix, session);
			beginning = edit.begin();
			return true;
		},
		queueKeys(targets: Map<NodeId, DragPreview>) {
			keys = targets;
			schedule();
		},
		queueBezier(targets: Map<NodeId, BezierEasingPreview>) {
			bezier = targets;
			schedule();
		},
		flush,
		discard,
		take() {
			const result = edit
				? { session: edit, begin: beginning, pending: context.writer.pending() }
				: null;
			edit = null;
			beginning = null;
			scope = null;
			return result;
		}
	};
}
