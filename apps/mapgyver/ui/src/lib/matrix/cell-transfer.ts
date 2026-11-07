import type { ContextMenuItem } from 'golden_ui';
import type { CellTransferAction } from '../media/generated/CellTransferAction';
import type { MatrixEdit } from '../media/generated/MatrixEdit';
import type { MediaEditResult } from '../media/generated/MediaEditResult';

export const CELL_DRAG_TYPE = 'application/x-mapgyver-matrix-cell';
export interface CellDrag {
	kind: 'Cell';
	project: string;
	media: string;
	cell: string;
	generation: number;
}
export function readCellDrag(text: string): CellDrag | null {
	if (!text || text.length > 2048) return null;
	try {
		const value = JSON.parse(text);
		if (
			value?.kind !== 'Cell' ||
			!Number.isSafeInteger(value.generation) ||
			value.generation < 0 ||
			!['project', 'media', 'cell'].every(
				(key) => typeof value[key] === 'string' && value[key].length > 0 && value[key].length <= 128
			)
		)
			return null;
		return {
			kind: 'Cell',
			project: value.project,
			media: value.media,
			cell: value.cell,
			generation: value.generation
		};
	} catch {
		return null;
	}
}
interface Host {
	status: string;
	snapshotGeneration: number;
	loading: { visible: boolean };
}
interface Target {
	project: string;
	media: string;
}
const clipboards = new WeakMap<object, { source: CellDrag; cut: boolean }>();

/** Input identities only. The backend owns drop defaults, occupied handling and subtree mutations. */
export function createCellTransfers<S extends Host>(options: {
	session(): S | null;
	target(): Target | null;
	busy(): boolean;
	edit(edit: MatrixEdit): Promise<MediaEditResult | undefined>;
	captureSelection(): (uuid: string) => boolean;
	selectAddress(layer: string, column: string): void;
}) {
	const capture = () => {
		const session = options.session(),
			target = options.target();
		return session && target
			? { session, ...target, generation: session.snapshotGeneration }
			: null;
	};
	type Token = ReturnType<typeof capture>;
	const current = (token: Token) =>
		!!token &&
		options.session() === token.session &&
		token.session.status === 'connected' &&
		!token.session.loading.visible &&
		token.session.snapshotGeneration === token.generation &&
		options.target()?.project === token.project &&
		options.target()?.media === token.media;
	const accepts = (source: CellDrag, token: Token) =>
		current(token) && source.project === token!.project && source.generation === token!.generation;
	async function run(
		source: CellDrag,
		layer: string,
		column: string,
		action: CellTransferAction,
		token = capture()
	) {
		if (!accepts(source, token) || options.busy()) return false;
		const select = options.captureSelection();
		const result = await options.edit({
			kind: 'TransferCell',
			cell: source.cell,
			layer,
			column,
			action
		});
		if (!result || !current(token) || !['Changed', 'MatrixCellPrepared'].includes(result.kind))
			return false;
		const cell = result.kind === 'MatrixCellPrepared' ? result.cell : source.cell;
		if (select(cell)) options.selectAddress(layer, column);
		return true;
	}
	function source(cell: string): CellDrag | null {
		const token = capture();
		return current(token) && !options.busy()
			? {
					kind: 'Cell',
					project: token!.project,
					media: token!.media,
					generation: token!.generation,
					cell
				}
			: null;
	}
	function clipboard() {
		const session = options.session(),
			token = capture();
		const entry = session ? clipboards.get(session) : undefined;
		if (entry && !accepts(entry.source, token)) {
			clipboards.delete(session!);
			return null;
		}
		return entry ?? null;
	}
	async function paste(layer: string, column: string, reference = false) {
		const session = options.session(),
			entry = clipboard();
		if (!session || !entry) return false;
		const action = reference ? 'Reference' : entry.cut ? 'Move' : 'Copy';
		const accepted = await run(entry.source, layer, column, action);
		if (accepted && action === 'Move' && clipboards.get(session) === entry)
			clipboards.delete(session);
		return accepted;
	}
	return {
		source,
		run,
		paste,
		copy(cell: string, cut = false) {
			const value = source(cell),
				session = options.session();
			if (!value || !session) return false;
			clipboards.set(session, { source: value, cut });
			return true;
		},
		menu(source: CellDrag, layer: string, column: string): ContextMenuItem[] {
			const token = capture();
			return (['Move', 'Copy', 'Reference'] as const).map((action) => ({
				id: `matrix-cell-${action.toLowerCase()}`,
				label: action,
				hint: 'Empty destination',
				disabled: !accepts(source, token) || options.busy(),
				action: async () => {
					await run(source, layer, column, action, token);
				}
			}));
		},
		clipboardActions(layer: string, column: string): ContextMenuItem[] {
			const token = capture();
			return [
				{
					label: 'Paste Cell',
					hint: 'Ctrl/Cmd+V',
					disabled: !clipboard(),
					action: async () => {
						if (current(token)) await paste(layer, column);
					}
				},
				{
					label: 'Paste Cell as Reference',
					disabled: !clipboard(),
					action: async () => {
						if (current(token)) await paste(layer, column, true);
					}
				}
			];
		}
	};
}
