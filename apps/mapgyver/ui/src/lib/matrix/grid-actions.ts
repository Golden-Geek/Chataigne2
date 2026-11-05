import type { ContextMenuItem } from 'golden_ui';
import removeIcon from 'golden_ui/style/icons/panels/close.svg';
import type { MatrixViewRow } from '../media/generated/MatrixViewRow';
import type { ColumnSnapshot } from '../media/generated/ColumnSnapshot';
import type { ColumnGroupSnapshot } from '../media/generated/ColumnGroupSnapshot';
import type { MatrixEdit } from '../media/generated/MatrixEdit';
import type { createMatrixEditor } from './editor.svelte';
import type { useDocumentSelection } from '../selection.svelte';

export function gridActions(
	owner: ReturnType<typeof createMatrixEditor>,
	selection: ReturnType<typeof useDocumentSelection>,
	row?: MatrixViewRow,
	column?: ColumnSnapshot,
	creationOnly = false,
	group?: ColumnGroupSnapshot,
	axis?: 'Column' | 'Layer'
): ContextMenuItem[] {
	const session = owner.session,
		generation = session?.snapshotGeneration,
		media = owner.view.media;
	const current = () =>
		owner.session === session &&
		session?.snapshotGeneration === generation &&
		owner.view.media === media &&
		!owner.busy;
	const run = async (edit: MatrixEdit) => {
		if (!current()) return;
		const select = selection.capture();
		const result = await owner.edit(edit);
		if (result?.kind === 'MatrixMediaCreated') select(result.media);
		if (result?.kind === 'MatrixColumnGroupCreated') {
			owner.controller.select(null, null);
			select(result.group);
		}
		if (result?.kind === 'MatrixCreated') {
			const item = result.items[0];
			const column = result.columns[0];
			if (item) {
				owner.controller.select(item, null);
				select(item.id);
			} else if (column) {
				owner.controller.select(null, column);
				select(column);
			}
		}
	};
	const action = (label: string, edit: MatrixEdit, icon?: string): ContextMenuItem => ({
		label,
		icon,
		action: () => run(edit)
	});
	if (axis) {
		const context = session?.getSelectedNodes()[0]?.uuid ?? null;
		return (
			axis === 'Column' ? (['Column', 'ColumnGroup'] as const) : (['Layer', 'LayerGroup'] as const)
		).map((item) =>
			action(
				item === 'ColumnGroup' ? 'Column Group' : item === 'LayerGroup' ? 'Layer Group' : item,
				{ kind: 'CreateAxisItem', item, context }
			)
		);
	}
	if (group)
		return [
			{
				label: 'Inspect Column Group settings',
				action: () => {
					if (current()) selection.select(group.id);
				}
			},
			action('Add Column in group', { kind: 'AddColumns', parent: group.id, count: 1 }),
			action('Add nested Column Group', { kind: 'AddColumnGroup', parent: group.id, label: null }),
			action('Move Column Group to root', {
				kind: 'DropColumnItem',
				item: { kind: 'Group', id: group.id },
				target: { kind: 'Root' }
			}),
			action('Ungroup Columns', { kind: 'UngroupColumns', group: group.id }),
			action('Duplicate Column Group', { kind: 'DuplicateColumnGroup', group: group.id }),
			action('Delete Column Group', { kind: 'RemoveColumnGroup', group: group.id }, removeIcon)
		];
	if (row?.item.id.kind === 'Layer' && column) {
		const layer = row.item.id.id;
		const cell = owner.matrixWindow?.cells.find(
			(cell) => cell.cell.layer === layer && cell.cell.definition.column === column.id
		)?.cell;
		const create = (owner.matrixWindow?.media_types ?? []).map((choice) =>
			action(choice.label, {
				kind: 'CreateCellMedia',
				layer,
				column: column.id,
				media_kind: choice.media_kind
			})
		);
		const inspect = {
			label: 'Inspect Cell settings',
			action: async () => {
				if (!current()) return;
				const select = selection.capture();
				const result = await owner.edit({ kind: 'EnsureCell', layer, column: column.id });
				if (result?.kind === 'MatrixCellPrepared') select(result.cell);
			}
		};
		if (creationOnly) return create;
		return [
			inspect,
			...(cell?.media
				? [
						{
							label: 'Inspect media',
							action: () => {
								if (current()) selection.select(cell.media!);
							}
						},
						action(
							'Remove media',
							{
								kind: 'SetCells',
								assignments: [{ layer, column: column.id, source: null }]
							},
							removeIcon
						)
					]
				: [{ label: 'Add Media', submenu: create }]),
			{ separator: true },
			action('Trigger Cell', { kind: 'LaunchCell', layer, column: column.id, mode: 'Configured' }),
			action('Cut Cell', { kind: 'LaunchCell', layer, column: column.id, mode: 'Cut' })
		];
	}
	if (row)
		return [
			{
				label: 'Inspect row settings',
				action: () => {
					if (current()) selection.select(row.item.id.id);
				}
			},
			...(row.item.id.kind === 'Group'
				? [
						action('Add layer in group', { kind: 'AddLayers', parent: row.item.id.id, count: 1 }),
						action('Add nested group', { kind: 'AddGroup', parent: row.item.id.id, label: null })
					]
				: []),
			action('Duplicate row', { kind: 'DuplicateItem', item: row.item.id }),
			action('Delete row', { kind: 'RemoveItem', item: row.item.id }, removeIcon)
		];
	if (column)
		return [
			{
				label: 'Inspect Column settings',
				action: () => {
					if (current()) selection.select(column.id);
				}
			},
			action('Launch Column', { kind: 'LaunchColumn', column: column.id, mode: 'Configured' }),
			action('Cut Column', { kind: 'LaunchColumn', column: column.id, mode: 'Cut' }),
			action('Duplicate Column', { kind: 'DuplicateColumn', column: column.id }),
			action('Delete Column', { kind: 'RemoveColumn', column: column.id }, removeIcon)
		];
	return [
		action('Add layer', { kind: 'AddLayers', parent: null, count: 1 }),
		action('Add group', { kind: 'AddGroup', parent: null, label: null }),
		action('Add column', { kind: 'AddColumns', parent: null, count: 1 }),
		action('Add Column Group', { kind: 'AddColumnGroup', parent: null, label: null }),
		action('Unload Matrix', { kind: 'ClearPlayback' })
	];
}
