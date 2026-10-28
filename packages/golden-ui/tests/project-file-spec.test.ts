import { describe, expect, it } from 'vitest';
import { parseProjectFileSpec } from '../project-file-spec';

describe('authoritative project file metadata', () => {
	it('preserves the app-published format and decodes an omitted optional path', () => {
		expect(parseProjectFileSpec({ display_name: 'MapGyver', extension: 'mapgyver' })).toEqual({
			display_name: 'MapGyver',
			extension: 'mapgyver',
			current_path: null
		});
		const spec = {
			display_name: 'Authored Project',
			extension: 'custom',
			current_path: '/work/show.custom'
		};
		expect(parseProjectFileSpec(spec)).toEqual(spec);
	});

	it('rejects absent or malformed metadata without substituting a format', () => {
		for (const value of [
			undefined,
			null,
			{},
			{ display_name: 'MapGyver' },
			{ extension: 'mapgyver' },
			{ display_name: '', extension: 'mapgyver' },
			{ display_name: ' ', extension: 'mapgyver' },
			{ display_name: 1, extension: 'mapgyver' },
			{ display_name: 'MapGyver', extension: '' },
			{ display_name: 'MapGyver', extension: '.mapgyver' },
			{ display_name: 'MapGyver', extension: ' mapgyver' },
			{ display_name: 'MapGyver', extension: 1 },
			{ display_name: 'MapGyver', extension: 'mapgyver', current_path: true },
			{ display_name: 'MapGyver', extension: 'mapgyver', current_path: '' }
		]) {
			expect(() => parseProjectFileSpec(value)).toThrow('Runtime project file metadata is invalid');
		}
	});
});
