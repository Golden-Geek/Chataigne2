import type { editorParameterProjection } from 'golden_ui/store/editor-parameter-projection';
import type { MatrixGeometryBindings } from '../media/generated/MatrixGeometryBindings';
import type { MatrixViewRow } from '../media/generated/MatrixViewRow';
import type { MatrixViewportSnapshot } from '../media/generated/MatrixViewportSnapshot';
import type { MatrixCellLayoutPage } from '../media/generated/MatrixCellLayoutPage';
import type { MatrixLayoutCell } from '../media/generated/MatrixLayoutCell';

type Parameters = ReturnType<typeof editorParameterProjection>;
type Bindings = ReadonlyMap<string, MatrixGeometryBindings>;

function row(value: MatrixViewRow, bindings: Bindings, parameters: Parameters): MatrixViewRow {
	const layer = value.item.layer,
		binding = bindings.get(value.item.id.id);
	if (!layer || !binding) return value;
	const size = parameters.value(binding.size, 'vec2', [layer.size.width, layer.size.height]);
	return {
		...value,
		item: {
			...value.item,
			layer: {
				...layer,
				position: parameters.value(binding.position, 'vec2', layer.position),
				size: { width: size[0], height: size[1] }
			}
		}
	};
}

function cell(
	value: MatrixLayoutCell,
	bindings: Bindings,
	parameters: Parameters
): MatrixLayoutCell {
	const settings = value.view.cell.definition.settings,
		binding = bindings.get(value.view.cell.id);
	if (!binding) return value;
	const stored = parameters.read(binding.size, 'vec2');
	const size = parameters.enabled(binding.size, settings.size !== null)
		? stored
			? { width: stored[0], height: stored[1] }
			: settings.size
		: null;
	return {
		...value,
		view: {
			...value.view,
			cell: {
				...value.view.cell,
				definition: {
					...value.view.cell.definition,
					settings: {
						...settings,
						position: parameters.value(binding.position, 'vec2', settings.position),
						size
					}
				}
			}
		}
	};
}

/** Only backend-bound typed Parameters are projected. Membership, source extent, natural
 * sizing and domain geometry remain owned by the existing authoritative DTOs. */
export function projectMatrixWindow(
	value: MatrixViewportSnapshot,
	bindings: Bindings,
	parameters: Parameters
): MatrixViewportSnapshot {
	return {
		...value,
		rows: value.rows.map((value) => row(value, bindings, parameters)),
		selected: value.selected
			? {
					...value.selected,
					row: value.selected.row ? row(value.selected.row, bindings, parameters) : null
				}
			: null
	};
}

export function projectCellLayoutPage(
	value: MatrixCellLayoutPage,
	parameters: Parameters
): MatrixCellLayoutPage {
	const bindings = new Map(value.parameters.map((binding) => [binding.item, binding]));
	return {
		...value,
		row: value.row ? row(value.row, bindings, parameters) : null,
		cells: value.cells.map((value) => cell(value, bindings, parameters)),
		selected: value.selected ? cell(value.selected, bindings, parameters) : null
	};
}
