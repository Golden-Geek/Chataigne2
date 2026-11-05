import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mediaOperationActivity } from '../operations.ts';

test('viewport and application inspections stay background while group authoring and launch retain user activity', () => {
	assert.equal(
		mediaOperationActivity({
			kind: 'Matrix',
			media: 'matrix',
			edit: {
				kind: 'InspectViewport',
				viewport: { row_start: 0, row_count: 1, axis_start: 0, axis_count: 1 },
				selection: null
			}
		}),
		'background'
	);
	assert.equal(
		mediaOperationActivity({
			kind: 'Applications',
			edit: { kind: 'Inspect', application: 'group' }
		}),
		'background'
	);
	assert.equal(
		mediaOperationActivity({
			kind: 'Matrix',
			media: 'matrix',
			edit: { kind: 'AddColumnGroup', parent: null, label: null }
		}),
		'user'
	);
	assert.equal(
		mediaOperationActivity({
			kind: 'Matrix',
			media: 'matrix',
			edit: { kind: 'LaunchColumn', column: 'column', mode: 'Cut' }
		}),
		'user'
	);
	assert.equal(
		mediaOperationActivity({
			kind: 'Applications',
			edit: { kind: 'SetFile', application: 'group', executable: null }
		}),
		'user'
	);
});
