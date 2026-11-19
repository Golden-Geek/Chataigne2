import fileIcon from 'golden_ui/style/icons/parameter/file.svg';
import { mediaFileDrop } from '../media/file-drop';
import type { ColumnItemId } from '../media/generated/ColumnItemId';
import type { MatrixFileTarget } from '../media/generated/MatrixFileTarget';

const executableTypes = new Set([
	'application/vnd.microsoft.portable-executable',
	'application/x-dosexec',
	'application/x-msdos-program',
	'application/x-msdownload'
]);

export function matrixFileDrop(
	element: Element,
	event: DragEvent,
	populated: (layer: string, column: string) => boolean,
	submit: (target: MatrixFileTarget, path: string) => Promise<void>
) {
	const heading = element.closest<HTMLElement>(
		'[data-matrix-drop="Column"], [data-matrix-drop="ColumnGroup"]'
	);
	const column = heading?.dataset.columnId;
	const group = heading?.dataset.columnGroupId;
	const item: ColumnItemId | undefined = group
		? { kind: 'Group', id: group }
		: column
			? { kind: 'Column', id: column }
			: undefined;
	if (heading && item) {
		return mediaFileDrop(
			{
				bounds: heading,
				label: item.kind === 'Group' ? 'Set Column Group App Control' : 'Set Column App Control',
				action: 'replace',
				icon: fileIcon
			},
			(path) => submit({ kind: 'ColumnItem', item }, path)
		);
	}
	const newColumn = element.closest<HTMLElement>('[data-matrix-file-new-column]');
	const newLayer = newColumn?.dataset.matrixFileNewColumn;
	if (newColumn && newLayer) {
		return mediaFileDrop({ bounds: newColumn, label: 'Add Column', action: 'add' }, (path) =>
			submit({ kind: 'NewColumn', layer: newLayer }, path)
		);
	}
	const cell = element.closest<HTMLElement>('[data-cell-drop]');
	const layer = cell?.dataset.cellLayer;
	const cellColumn = cell?.dataset.cellColumn;
	if (!cell || !layer || !cellColumn) return null;
	const file = event.dataTransfer?.files[0];
	const type = file?.type || event.dataTransfer?.items[0]?.type || '';
	// File names are only exposed on drop. A MIME type can supply a hover hint.
	const executable = file ? /\.exe$/i.test(file.name) : executableTypes.has(type);
	const unknown = !file && (!type || type === 'application/octet-stream');
	const replaces = populated(layer, cellColumn);
	const label = executable
		? 'Set Cell App Control'
		: unknown
			? 'Import media or set App Control'
			: replaces
				? 'Replace Cell media'
				: 'Import into Cell';
	return mediaFileDrop(
		{
			bounds: cell,
			label,
			action: executable || replaces ? 'replace' : 'add',
			icon: executable ? fileIcon : undefined
		},
		(path) => submit({ kind: 'Cell', layer, column: cellColumn }, path)
	);
}
