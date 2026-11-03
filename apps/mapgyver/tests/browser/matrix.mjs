// Current Matrix acceptance composes the focused real-input flows rather than obsolete forms.
import assert from 'node:assert/strict';
import { verifyMatrixDefaultsUi } from './matrix-defaults.mjs';
import { verifyMatrixGridUi } from './matrix-grid.mjs';
import { verifyMatrixDragUi } from './matrix-drag.mjs';
import { verifyMatrixParametersUi } from './matrix-parameters.mjs';
import { verifyMatrixOwnedMediaUi } from './matrix-owned-media.mjs';
import { verifyMatrixViewportUi } from './matrix-viewport.mjs';

export async function verifyMatrixUi(options) {
	const flows = [
		['3x3 catalog/defaults/history', verifyMatrixDefaultsUi],
		['square grid/context creation/rename/structure/history', verifyMatrixGridUi],
		['row/group/Column drag and virtualized source lifetime', verifyMatrixDragUi],
		['typed settings and offscreen shared Inspector/history', verifyMatrixParametersUi],
		[
			'Reference/Alt-copy, cyclic rejection, missing/recovery and replacement',
			verifyMatrixOwnedMediaUi
		],
		['keyboard/resize/tab lifetime and narrow Inspector', verifyMatrixViewportUi]
	];
	for (const [name, flow] of flows) {
		assert.equal((await options.post('/api/ui/project-new', {})).ok, true);
		await flow(options);
		console.log(JSON.stringify({ matrixChecklist: name, result: 'pass' }));
	}
	console.log(
		JSON.stringify({
			result: 'pass',
			scope:
				'Complete revised Matrix interaction checklist; project replacement and actual native feedback have separate owning flows',
			checklist: flows.map(([name]) => name)
		})
	);
}
