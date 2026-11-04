import { MAX_REGION_TILES } from 'golden_video_ui/constants';
import type { PreviewTileGroup } from 'golden_video_ui';
import type { MatrixViewportSnapshot } from '../media/generated/MatrixViewportSnapshot';
import { gridSize, projectGrid, type Metrics } from './viewport.ts';

export type CellPreviewGroup = PreviewTileGroup;

/** Viewport presentation only. Effective channel identities and labels come from the backend. */
export function cellPreviewGroups(
	snapshot: MatrixViewportSnapshot,
	metrics: Metrics
): CellPreviewGroup[] {
	const projection = projectGrid(metrics, snapshot.row_total, snapshot.column_total);
	const rows = new Map(
		snapshot.rows.map((row, index) => [row.item.id.id, snapshot.row_start + index])
	);
	const columns = new Map(
		snapshot.columns.map((column, index) => [column.id, snapshot.column_start + index])
	);
	const groups = new Map<string, CellPreviewGroup>();
	const rem = metrics.rem > 0 ? metrics.rem : 1;
	const width = metrics.width - gridSize.label * rem;
	const height = metrics.height - gridSize.header * rem;
	if (width <= 0 || height <= 0) return [];
	let count = 0;
	for (const cell of snapshot.cells) {
		if (!cell.source_preview || cell.source_missing) continue;
		const row = rows.get(cell.cell.layer);
		const column = columns.get(cell.cell.definition.column);
		if (row === undefined || column === undefined) continue;
		const tile = {
			x:
				(projection.columnOrigin +
					column * gridSize.column -
					projection.left -
					gridSize.label +
					0.2) *
				rem,
			y: (projection.rowOrigin + row * gridSize.row - projection.top - gridSize.header + 0.2) * rem,
			width: (gridSize.column - 0.4) * rem,
			height: (gridSize.row - 1.8) * rem
		};
		if (
			tile.x + tile.width <= 0 ||
			tile.y + tile.height <= 0 ||
			tile.x >= width ||
			tile.y >= height
		)
			continue;
		if (++count > MAX_REGION_TILES)
			throw new Error('Matrix preview geometry exceeds admitted capacity');
		const source = JSON.stringify({ kind: 'Channel', reference: cell.source_preview });
		let group = groups.get(source);
		if (!group) {
			group = { source, tiles: [], capacity: 1 };
			groups.set(source, group);
		}
		group.tiles.push(tile);
		group.capacity = 2 ** Math.ceil(Math.log2(group.tiles.length));
	}
	return [...groups.values()].sort((left, right) => left.source.localeCompare(right.source));
}
