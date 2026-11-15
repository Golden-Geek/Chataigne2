import {
	createEditorOperationScopes,
	type EditorOperation
} from '../../../store/editor-operation-scope';
import { createUiEditSession, sendFitAnimationCurvePathIntent } from '../../../store/ui-intents';
import type { WorkbenchSession } from '../../../store/workbench.svelte';
import type {
	UiNodeDto,
	UiAnimationCurveFitPoint,
	UiAnimationCurveBezierFitOptions
} from '../../../types';

export interface CurveOperation extends EditorOperation<WorkbenchSession> {
	readonly owner: { node_id: number; uuid: string };
}

export function createCurveOperations(context: {
	readonly session: WorkbenchSession | null;
	readonly owner: UiNodeDto;
}) {
	const scopes = createEditorOperationScopes(() => context.session);
	const capture = (): CurveOperation | null => {
		const host = context.session,
			owner = { node_id: context.owner.node_id, uuid: context.owner.uuid };
		if (!host) return null;
		const lease = scopes.capture(
			() => context.owner.node_id === owner.node_id && context.owner.uuid === owner.uuid,
			() => host.graph.state.nodesById.get(owner.node_id)?.uuid === owner.uuid
		);
		return lease ? { ...lease, owner } : null;
	};
	return {
		capture,
		observe: scopes.observe,
		dispose: scopes.dispose,
		async draw(
			points: UiAnimationCurveFitPoint[],
			options: UiAnimationCurveBezierFitOptions
		): Promise<boolean> {
			const scope = capture();
			if (!scope || points.length < 2) {
				scope?.release();
				return false;
			}
			const group = createUiEditSession('Draw Curve', 'curve-draw', scope.host);
			try {
				await group.begin();
				if (!scope.current() || !group.active) return false;
				return await sendFitAnimationCurvePathIntent(scope.owner.node_id, points, options);
			} finally {
				try {
					await group.end();
				} finally {
					scope.release();
				}
			}
		}
	};
}
