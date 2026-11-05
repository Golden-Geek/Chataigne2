import type { MatrixSelection } from '../media/generated/MatrixSelection';
import type { MatrixViewport } from '../media/generated/MatrixViewport';
export interface MatrixPanelState {
	document: string | null;
	media: string | null;
	locked: boolean;
	selection: MatrixSelection | null;
	node: string | null;
	viewport: MatrixViewport;
	scroll: { top: number; left: number };
}
const uuid = (value: unknown): value is string =>
	typeof value === 'string' && /^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(value);
const integer = (value: unknown, maximum: number, fallback = 0) =>
	typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
		? Math.min(maximum, value)
		: fallback;
const offset = (value: unknown) =>
	typeof value === 'number' && Number.isFinite(value) && value >= 0 ? Math.min(50000, value) : 0;
export function matrixPanelState(value: Partial<MatrixPanelState> = {}): MatrixPanelState {
	const valid = uuid(value.document) && uuid(value.media),
		item = value.selection?.item;
	return {
		document: valid ? value.document! : null,
		media: valid ? value.media! : null,
		locked: valid && value.locked === true,
		selection: value.selection
			? {
					item:
						item && (item.kind === 'Layer' || item.kind === 'Group') && uuid(item.id)
							? { ...item }
							: null,
					column: uuid(value.selection.column) ? value.selection.column : null
				}
			: null,
		node: valid && uuid(value.node) ? value.node : null,
		viewport: {
			row_start: integer(value.viewport?.row_start, 0xffffffff),
			axis_start: integer(value.viewport?.axis_start, 0xffffffff),
			row_count: Math.max(1, integer(value.viewport?.row_count, 64, 32)),
			axis_count: Math.max(1, integer(value.viewport?.axis_count, 32, 16))
		},
		scroll: { top: offset(value.scroll?.top), left: offset(value.scroll?.left) }
	};
}
