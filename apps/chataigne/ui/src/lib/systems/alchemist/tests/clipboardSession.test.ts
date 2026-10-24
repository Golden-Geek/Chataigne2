import { describe, expect, it } from 'vitest';
import { pasteFormulaClipboard } from '../alchemistClipboard';
import { clipboardNode, clipboardSession } from './fixtures/clipboardSession';

const request = {
	formula: { node: 42, uuid: 'formula' },
	fragment: { version: 1, source_formula_uuid: 'source', items: [] },
	offset: null
};
const ack = {
	success: true,
	status: 'applied' as const,
	result: {
		value: {
			selected_nodes: [90, 91],
			identities: [
				{ source_uuid: 'a', node: 90, uuid: 'created-a' },
				{ source_uuid: 'b', node: 91, uuid: 'created-b' }
			]
		}
	}
};

describe('Formula paste through the real workbench queue and projection', () => {
	it('selects the exact reply only after complete projection, ignoring independent creates', async () => {
		const controlled = await clipboardSession();
		try {
			const paste = pasteFormulaClipboard(controlled.session, request);
			controlled.respond(ack);
			await paste;
			expect(controlled.session.selectedNodesIds).toEqual([]);
			const revision = controlled.session.selectionRevision;
			controlled.emit(clipboardNode(89, 'independent-create'));
			controlled.emit(clipboardNode(91, 'created-b'));
			expect(controlled.session.selectedNodesIds).toEqual([]);
			controlled.emit(clipboardNode(90, 'created-a'));
			expect(controlled.session.selectedNodesIds).toEqual([90, 91]);
			expect(controlled.session.selectionRevision).toBe(revision);
		} finally {
			controlled.dispose();
		}
	});
	it.each(['before acknowledgement', 'before projection'])(
		'preserves a newer user selection made %s',
		async (when) => {
			const controlled = await clipboardSession();
			try {
				const paste = pasteFormulaClipboard(controlled.session, request);
				if (when === 'before acknowledgement') controlled.session.selectNode(1);
				controlled.respond(ack);
				await paste;
				if (when === 'before projection') controlled.session.selectNode(1);
				controlled.emit(clipboardNode(90, 'created-a'));
				controlled.emit(clipboardNode(91, 'created-b'));
				expect(controlled.session.selectedNodesIds).toEqual([1]);
			} finally {
				controlled.dispose();
			}
		}
	);
	it('rejects a reused projected ID instead of selecting the wrong node', async () => {
		const controlled = await clipboardSession();
		try {
			const paste = pasteFormulaClipboard(controlled.session, request);
			controlled.respond(ack);
			await paste;
			controlled.emit(clipboardNode(90, 'unrelated-reused-id'));
			controlled.emit(clipboardNode(91, 'created-b'));
			expect(controlled.session.selectedNodesIds).toEqual([]);
		} finally {
			controlled.dispose();
		}
	});
	it('retires a pending selection through the public project-replacement barrier', async () => {
		const controlled = await clipboardSession();
		try {
			const paste = pasteFormulaClipboard(controlled.session, request);
			controlled.respond(ack);
			await paste;
			const root = { ...clipboardNode(1, 'replacement-root'), children: [90, 91] };
			await controlled.replaceProject([
				root,
				clipboardNode(90, 'different-a'),
				clipboardNode(91, 'different-b')
			]);
			expect(controlled.session.graph.state.nodesById.get(1)?.uuid).toBe('replacement-root');
			expect(controlled.session.selectedNodesIds).toEqual([]);
		} finally {
			controlled.dispose();
		}
	});
});
