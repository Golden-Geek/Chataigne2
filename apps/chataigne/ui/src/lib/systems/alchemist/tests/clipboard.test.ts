import { describe, expect, it, vi } from 'vitest';
import {
	findFormulaPasteOffset,
	formulaFragmentFromJson,
	formulaFragmentJson,
	pasteFormulaClipboard
} from '../alchemistClipboard';
import type { FormulaFragment } from '../generated/authoring/FormulaFragment';

const fragment: FormulaFragment = {
	version: 1,
	source_formula_uuid: 'source-formula',
	items: [
		{
			document: { opaque: { children: [{ hidden: 'complete authored child' }] } },
			position: [3, 4]
		}
	]
};
const request = { formula: { node: 42, uuid: 'destination' }, fragment, offset: null };
const result = {
	selected_nodes: [809, 810],
	identities: [
		{ source_uuid: 'source-b', node: 810, uuid: 'created-b' },
		{ source_uuid: 'source-a', node: 809, uuid: 'created-a' },
		{ source_uuid: 'child', node: 811, uuid: 'created-child' }
	]
};

describe('backend-owned Formula clipboard workflow', () => {
	it('round trips complete opaque capsules without projecting or repairing children', () => {
		expect(formulaFragmentFromJson(formulaFragmentJson(fragment))).toEqual(fragment);
	});
	it.each([
		'not json',
		'{}',
		'{"kind":"chataigne.alchemist.nodes","version":2,"clipboard":{}}',
		JSON.stringify({ ...fragment, version: 999 })
	])('rejects unsupported clipboard envelopes: %s', (text) => {
		expect(formulaFragmentFromJson(text)).toBeNull();
	});
	it('requests exact root selection from the reply without waiting for projected children', async () => {
		const session = {
			graphTransitionRevision: 0,
			selectionRevision: 0,
			invokeAppOperation: vi.fn().mockResolvedValue(result),
			requestNodeSelection: vi.fn()
		};
		await pasteFormulaClipboard(session, request);
		expect(session.invokeAppOperation).toHaveBeenCalledOnce();
		expect(session.requestNodeSelection).toHaveBeenCalledWith([
			{ node_id: 809, uuid: 'created-a' },
			{ node_id: 810, uuid: 'created-b' }
		]);
	});
	it.each(['project replacement', 'view changed', 'user selection'])(
		'does not steal selection after %s',
		async (change) => {
			let release!: (value: typeof result) => void;
			const pending = new Promise<typeof result>((resolve) => {
				release = resolve;
			});
			const session = {
				graphTransitionRevision: 0,
				selectionRevision: 0,
				invokeAppOperation: vi.fn().mockReturnValue(pending),
				requestNodeSelection: vi.fn()
			};
			let current = true;
			const done = pasteFormulaClipboard(session, request, () => current);
			if (change === 'project replacement') session.graphTransitionRevision++;
			else if (change === 'user selection') session.selectionRevision++;
			else current = false;
			release(result);
			await done;
			expect(session.requestNodeSelection).not.toHaveBeenCalled();
		}
	);
	it('does not select after atomic rejection or invent a missing result identity', async () => {
		const session = {
			graphTransitionRevision: 0,
			selectionRevision: 0,
			invokeAppOperation: vi.fn().mockRejectedValue(new Error('invalid fragment')),
			requestNodeSelection: vi.fn()
		};
		await expect(pasteFormulaClipboard(session, request)).rejects.toThrow('invalid fragment');
		expect(session.requestNodeSelection).not.toHaveBeenCalled();
		session.invokeAppOperation.mockResolvedValue({ selected_nodes: [999], identities: [] });
		await expect(pasteFormulaClipboard(session, request)).rejects.toThrow('authoritative identity');
		expect(session.requestNodeSelection).not.toHaveBeenCalled();
	});
	it('centers cross-Formula placement using public positions without interpreting document capsules', () => {
		expect(
			findFormulaPasteOffset({
				fragment,
				occupied: [],
				viewportCenter: { x: 100, y: 50 },
				destinationUuid: 'other'
			})
		).toEqual([90.5, 43.75]);
	});
});
