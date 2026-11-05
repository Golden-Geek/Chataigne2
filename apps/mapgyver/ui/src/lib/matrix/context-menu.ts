import { appState, registerNodeContextMenuContributor } from 'golden_ui';
import deleteIcon from 'golden_ui/style/icons/panels/close.svg';
import { invokeMediaEdit } from '../media/operations';
import type { MatrixEdit } from '../media/generated/MatrixEdit';

/** Paired Matrix rows/exports are removed by the same backend operation from every tree. */
export function registerMatrixContextMenu() {
	registerNodeContextMenuContributor('mapgyver.matrix-removal', {
		match: ({ node }) =>
			[
				'mapgyver.matrix.layer.v3',
				'mapgyver.matrix.group.v2',
				'mapgyver.matrix.column_group.v1',
				'mapgyver.matrix.column.v2'
			].includes(node.node_type),
		items: ({ node, graphState, session, closeMenu }) => {
			let owner = node;
			while (owner.node_type !== 'mapgyver.media.v8') {
				const parent = graphState?.parentById.get(owner.node_id);
				const next = parent == null ? null : graphState?.nodesById.get(parent);
				if (!next) return [];
				owner = next;
			}
			const media = owner.uuid;
			const generation = session?.snapshotGeneration;
			const edit: MatrixEdit =
				node.node_type === 'mapgyver.matrix.column_group.v1'
					? { kind: 'RemoveColumnGroup', group: node.uuid }
					: node.node_type === 'mapgyver.matrix.column.v2'
						? { kind: 'RemoveColumn', column: node.uuid }
						: {
								kind: 'RemoveItem',
								item: {
									kind: node.node_type === 'mapgyver.matrix.layer.v3' ? 'Layer' : 'Group',
									id: node.uuid
								}
							};
			return [
				{
					id: 'mapgyver-remove-matrix-object',
					label: 'Delete',
					icon: deleteIcon,
					disabled:
						session?.status !== 'connected' ||
						session.loading.visible ||
						session.hasActiveEditSession,
					action: () => {
						closeMenu();
						if (
							!session ||
							appState.session !== session ||
							session.snapshotGeneration !== generation ||
							session.getNodeData(node.node_id)?.uuid !== node.uuid
						)
							return;
						void invokeMediaEdit(session, { kind: 'Matrix', media, edit }).catch((error) =>
							console.error('Matrix removal rejected', error)
						);
					}
				}
			];
		}
	});
}
