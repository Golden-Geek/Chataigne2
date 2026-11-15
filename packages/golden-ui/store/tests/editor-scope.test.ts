import { test } from 'vitest';
import assert from 'node:assert/strict';
import { createEditorScopeObserver, type EditorScope } from '../editor-scope';

test('fresh reactive wrappers preserve an admitted gesture until its semantic lifetime changes', () => {
	let captured = true,
		cancellations = 0;
	const observe = createEditorScopeObserver(() => {
		captured = false;
		cancellations++;
	});
	const scope: EditorScope = {
		host: {},
		project: 1,
		snapshot: 1,
		status: 'connected',
		loading: false,
		owner: 'curve'
	};
	observe(scope);
	for (let frame = 0; frame < 30; frame++) observe({ ...scope });
	assert(captured);
	assert.equal(cancellations, 0);
	observe({ ...scope, snapshot: 2 });
	assert.equal(captured, false);
	assert.equal(cancellations, 1);
	observe({ ...scope, snapshot: 2 });
	assert.equal(cancellations, 1);
});

for (const field of ['host', 'project', 'snapshot', 'status', 'loading', 'owner'] as const) {
	test(`${field} replacement retires exactly one input lifetime`, () => {
		let cancellations = 0;
		const observe = createEditorScopeObserver(() => cancellations++);
		const scope: EditorScope = {
			host: {},
			project: 1,
			snapshot: 1,
			status: 'connected',
			loading: false,
			owner: 'curve'
		};
		observe(scope);
		const replacement = {
			...scope,
			[field]: {
				host: {},
				project: 2,
				snapshot: 2,
				status: 'disconnected',
				loading: true,
				owner: null
			}[field]
		};
		observe(replacement);
		observe({ ...replacement });
		assert.equal(cancellations, 1);
	});
}
