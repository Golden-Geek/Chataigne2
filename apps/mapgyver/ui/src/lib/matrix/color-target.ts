import {
	nodeColorTarget,
	type NodeColorTarget
} from 'golden_ui/components/common/node-color-menu.ts';
import type { createMatrixEditor } from './editor.svelte';
import { invokeMediaEdit } from '../media/operations';

export function matrixColorTarget(
	owner: ReturnType<typeof createMatrixEditor>,
	address: string | { layer: string; column: string }
): NodeColorTarget | null {
	const session = owner.session,
		generation = session?.snapshotGeneration,
		media = owner.view.media;
	if (!session || !media) return null;
	const node = (uuid: string) => session.graph.state.nodesById.getByUuid(uuid);
	if (typeof address === 'string') {
		const target = node(address);
		return target ? nodeColorTarget(session, target) : null;
	}
	let cellUuid: string | null = null;
	const current = () => {
		if (cellUuid) return node(cellUuid);
		const cell = owner.matrixWindow?.cells.find(
			(entry) =>
				entry.cell.layer === address.layer && entry.cell.definition.column === address.column
		)?.cell;
		cellUuid = cell?.id ?? null;
		return cell ? node(cell.id) : null;
	};
	current();
	return {
		valid: () =>
			owner.session === session &&
			session.status === 'connected' &&
			!session.loading.visible &&
			session.snapshotGeneration === generation &&
			owner.view.media === media &&
			!!node(address.layer) &&
			!!node(address.column),
		read: () =>
			current()?.meta.presentation?.color ??
			current()?.meta.presentation?.default_color ?? { r: 1, g: 1, b: 1, a: 1 },
		hasOverride: () => !!current()?.meta.presentation?.color,
		async write(color) {
			if (!this.valid()) return;
			const result = await invokeMediaEdit(session, {
				kind: 'Matrix',
				media,
				edit: { kind: 'SetCellColor', ...address, color }
			});
			if (result.kind === 'MatrixCellColorSet') cellUuid = result.cell;
		}
	};
}
