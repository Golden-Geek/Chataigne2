import type { UiNodeDto } from '../../../../types';
import {
	createInspectorEditorSelection,
	type InspectorEditorSelectionHost
} from '../editor-selection.svelte';
import {
	createCurveSelectionProjection,
	curveSelectionNodes,
	curveSelectionSignature,
	type CurveSelection
} from './curve-selection';

export function createCurveInspectorSelection(context: {
	readonly session: InspectorEditorSelectionHost | null;
	readonly owner: UiNodeDto;
	read(): CurveSelection;
	write(selection: CurveSelection): void;
}) {
	const projection = createCurveSelectionProjection();
	return createInspectorEditorSelection({
		get session() {
			return context.session;
		},
		get owner() {
			return context.owner;
		},
		read: context.read,
		write: context.write,
		empty: () => ({ key: null, keys: [], segments: [] }),
		signature: curveSelectionSignature,
		project: (graph, ids, owner) => projection.read(graph, ids, owner.node_id, owner.uuid),
		resolve: curveSelectionNodes,
		retainPrimary: (selected, current) => ({
			...selected,
			key: current.key !== null && selected.keys.includes(current.key) ? current.key : selected.key
		})
	});
}
