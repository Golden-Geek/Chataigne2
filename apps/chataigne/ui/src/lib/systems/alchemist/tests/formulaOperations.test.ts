import { describe, expect, it, vi } from 'vitest';
import { copyFormulaFragment, pasteFormulaFragment } from '../formulaOperations';
import type { FormulaFragment } from '../generated/authoring/FormulaFragment';
import type { FormulaPasteResult } from '../generated/authoring/FormulaPasteResult';

const formula = { node: 42, uuid: 'destination-formula' };
const fragment: FormulaFragment = {
	version: 1,
	source_formula_uuid: 'source-formula',
	items: [{ document: { root: { opaque: 'authored content' } }, position: [3, 4] }]
};

describe('Formula operation adapter', () => {
	it('retains complete copy capsules without consulting the projected graph', async () => {
		const session = { invokeAppOperation: vi.fn().mockResolvedValue(fragment) };
		expect(await copyFormulaFragment(session, { formula, nodes: [10, 11] })).toBe(fragment);
		expect(session.invokeAppOperation).toHaveBeenCalledWith({
			kind: 'invokeAppOperation',
			namespace: 'chataigne.alchemist',
			operation: 'copyFragment',
			payload: { formula, nodes: [10, 11] }
		});
	});

	it('waits for the authoritative paste identities independently of presentation', async () => {
		const result: FormulaPasteResult = {
			selected_nodes: [809],
			identities: [{ source_uuid: 'source', uuid: 'created', node: 809 }]
		};
		let complete: (value: FormulaPasteResult) => void = () => {};
		const pending = new Promise<FormulaPasteResult>((resolve) => (complete = resolve));
		const session = { invokeAppOperation: vi.fn().mockReturnValue(pending) };
		const request = { formula, fragment, offset: [5, 7] as [number, number] };
		const reply = pasteFormulaFragment(session, request);
		complete(result);
		expect(await reply).toBe(result);
		expect(session.invokeAppOperation).toHaveBeenCalledWith({
			kind: 'invokeAppOperation',
			namespace: 'chataigne.alchemist',
			operation: 'pasteFragment',
			payload: request
		});
	});

	it.each([null, {}, { selected_nodes: [809], identities: [{ node: 809 }] }])(
		'rejects malformed paste results without inventing identities: %j',
		async (result) => {
			const session = { invokeAppOperation: vi.fn().mockResolvedValue(result) };
			await expect(
				pasteFormulaFragment(session, { formula, fragment, offset: null })
			).rejects.toThrow('invalid result');
		}
	);

	it('preserves an atomic backend rejection', async () => {
		const session = {
			invokeAppOperation: vi.fn().mockRejectedValue(new Error('Formula identity changed'))
		};
		await expect(
			pasteFormulaFragment(session, { formula, fragment, offset: null })
		).rejects.toThrow('Formula identity changed');
	});
});
