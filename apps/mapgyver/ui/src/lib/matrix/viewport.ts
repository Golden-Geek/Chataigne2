import type { MatrixViewport } from '../media/generated/MatrixViewport';
/** Persisted state and generated replies may enumerate these fields in different orders. */
export function sameViewport(left: MatrixViewport, right: MatrixViewport): boolean {
	return (
		left.row_start === right.row_start &&
		left.axis_start === right.axis_start &&
		left.row_count === right.row_count &&
		left.axis_count === right.axis_count
	);
}
export const gridSize = {
	row: 7.5,
	column: 7.5,
	label: 14,
	header: 3.5,
	previewLabel: 2.2,
	creation: 2.25,
	maximum: 50000
};
export const groupHeaderRow = 1.8;
export function groupHeaderHeight(metrics: Metrics, depth: number) {
	return Math.min(
		depth * groupHeaderRow,
		8,
		Math.max(0, metrics.height / Math.max(metrics.rem, 1)) * 0.3
	);
}
export interface Metrics {
	top: number;
	left: number;
	width: number;
	height: number;
	rem: number;
}

/** Small tables fill the viewport; larger tables keep readable minimum spans and bounded windows. */
export function projectGrid(metrics: Metrics, rows: number, columns: number, groupDepth = 0) {
	const { label, maximum, creation } = gridSize;
	const groupHeight = groupHeaderHeight(metrics, groupDepth);
	const header = gridSize.header + groupHeight;
	const rem = metrics.rem > 0 ? metrics.rem : 1;
	const width = metrics.width / rem,
		height = metrics.height / rem;
	const column = Math.max(gridSize.column, (width - label - creation) / Math.max(1, columns));
	const row = Math.max(gridSize.row, (height - header - creation) / Math.max(1, rows));
	const logicalWidth = label + columns * column + creation,
		logicalHeight = header + rows * row + creation;
	const canvasWidth = Math.min(maximum, logicalWidth),
		canvasHeight = Math.min(maximum, logicalHeight);
	const top = Math.max(0, metrics.top / rem),
		left = Math.max(0, metrics.left / rem);
	const logicalTop =
		(top / Math.max(1, canvasHeight - height)) * Math.max(0, logicalHeight - height);
	const logicalLeft = (left / Math.max(1, canvasWidth - width)) * Math.max(0, logicalWidth - width);
	const viewport: MatrixViewport = {
		row_start: Math.min(
			Math.max(0, rows - 1),
			Math.max(0, Math.floor((logicalTop - header) / row) - 2)
		),
		row_count: Math.min(64, Math.max(1, Math.ceil(height / row) + 4)),
		axis_start: Math.min(
			Math.max(0, columns - 1),
			Math.max(0, Math.floor((logicalLeft - label) / column) - 2)
		),
		axis_count: Math.min(32, Math.max(1, Math.ceil(width / column) + 4))
	};
	return {
		viewport,
		row,
		column,
		header,
		groupHeight,
		canvasWidth,
		canvasHeight,
		top,
		left,
		rowOrigin: header + top - logicalTop,
		columnOrigin: label + left - logicalLeft
	};
}
