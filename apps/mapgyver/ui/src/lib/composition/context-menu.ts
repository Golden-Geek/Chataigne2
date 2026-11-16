import { appState, registerNodeContextMenuContributor } from 'golden_ui';
import { invokeMediaEdit } from '../media/operations';
import type { CompositionEdit } from '../media/generated/CompositionEdit';

/** Extend the ordinary node menu; ownership and dimensions are resolved by the backend. */
export function registerCompositionContextMenu() {
	registerNodeContextMenuContributor('mapgyver.composition-transform', {
		match: ({ node, graphState }) =>
			(node.children ?? []).some(
				(id) => graphState?.nodesById.get(id)?.node_type === 'mapgyver.composition.placement.v3'
			),
		items: ({ node, session, closeMenu }) => {
			const generation = session?.snapshotGeneration;
			const current = () =>
				!!session &&
				appState.session === session &&
				session.status === 'connected' &&
				!session.loading.visible &&
				!session.hasActiveEditSession &&
				session.snapshotGeneration === generation &&
				session.getNodeData(node.node_id)?.uuid === node.uuid;
			const apply = async (
				edit:
					| Omit<Extract<CompositionEdit, { kind: 'TransformInstance' }>, 'instance'>
					| Omit<Extract<CompositionEdit, { kind: 'ArrangeInstance' }>, 'instance'>
			) => {
				closeMenu();
				if (!session || !current()) return;
				try {
					const result = await invokeMediaEdit(session, {
						kind: 'InspectCompositionEditor',
						query: {
							selected: node.uuid,
							document: null,
							locked: null,
							retained: null,
							instance: null,
							start: 0,
							count: 1
						}
					});
					if (
						!current() ||
						result.kind !== 'CompositionEditor' ||
						result.page.selected?.reference.instance !== node.uuid
					)
						return;
					await invokeMediaEdit(session, {
						kind: 'Composition',
						reference: result.page.selected.reference.composition,
						edit: { ...edit, instance: node.uuid }
					});
				} catch (error) {
					console.error('Composition action rejected', error);
				}
			};
			return [
				{
					id: 'mapgyver-composition-arrange',
					label: 'Arrange',
					disabled: !current(),
					submenu: (['Back', 'Front'] as const).map((action) => ({
						label: `To ${action}`,
						action: () => apply({ kind: 'ArrangeInstance', action })
					}))
				},
				{
					id: 'mapgyver-composition-transform',
					label: 'Transform',
					disabled: !current(),
					submenu: (['Reset', 'Fit', 'Fill'] as const).map((action) => ({
						label: action,
						action: () => apply({ kind: 'TransformInstance', action })
					}))
				}
			];
		}
	});
}
