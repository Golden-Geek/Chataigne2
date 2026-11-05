import assert from 'node:assert/strict';
import { test } from 'node:test';
import { cellExtent } from '../rect-presentation.ts';
import type { MatrixLayoutCell } from '../../media/generated/MatrixLayoutCell';

const cell = (
	size: { width: number; height: number } | null,
	extent: { width: number; height: number } | null
): MatrixLayoutCell => ({
	label: 'Cell',
	extent,
	preview: { kind: 'MatrixCell', media: 'matrix', cell: 'cell' },
	view: {
		cell: {
			id: 'cell',
			media: null,
			layer: 'layer',
			definition: {
				application: 'application',
				transition: null,
				column: 'column',
				source: null,
				settings: { position: [0, 0], size, fit_mode: 'Fit', opacity: 1, enabled: false }
			}
		},
		source_label: null,
		source_preview: null,
		source_missing: false
	}
});

test('natural Cell bounds follow fresh renderer dimensions while authored Size stays independent', () => {
	const old = { width: 8, height: 4 },
		fresh = { width: 16, height: 9 },
		fixed = { width: 12, height: 12 };
	assert.deepEqual(cellExtent(cell(null, old), fresh), fresh);
	assert.deepEqual(cellExtent(cell(fixed, old), fresh), fixed);
	assert.deepEqual(cellExtent(cell(null, old)), old);
	assert.equal(
		cellExtent(cell(null, null)),
		null,
		'unknown dimensions never acquire a synthetic authored size'
	);
});
