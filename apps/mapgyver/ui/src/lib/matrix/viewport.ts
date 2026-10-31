import type { MatrixViewport } from '../media/generated/MatrixViewport';
export const gridSize = { row: 2.8, column: 10, label: 14, header: 5.5, maximum: 50000 };
export interface Metrics {
	top: number;
	left: number;
	width: number;
	height: number;
	rem: number;
}
/** Rem-based geometry with compressed scroll ranges below browser layout limits. */
export function projectGrid(metrics: Metrics, rows: number, columns: number) {
	const { row, column, label, header, maximum } = gridSize;
	const rem = metrics.rem > 0 ? metrics.rem : 1;
	const width = metrics.width / rem,
		height = metrics.height / rem;
	const logicalWidth = label + columns * column,
		logicalHeight = header + rows * row;
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
		column_start: Math.min(
			Math.max(0, columns - 1),
			Math.max(0, Math.floor((logicalLeft - label) / column) - 2)
		),
		column_count: Math.min(32, Math.max(1, Math.ceil(width / column) + 4))
	};
	return {
		viewport,
		canvasWidth,
		canvasHeight,
		top,
		left,
		rowOrigin: header + top - logicalTop,
		columnOrigin: label + left - logicalLeft
	};
}
