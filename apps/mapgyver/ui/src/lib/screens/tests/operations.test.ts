import { test } from 'node:test';
import assert from 'node:assert/strict';
import { invokeScreenEdit, screenOperationActivity } from '../operations.ts';
import type { ScreenEdit } from '../generated/ScreenEdit';

test('each independent editor refresh stays background through the ordinary invocation boundary', async () => {
	for (const editor of ['Screen', 'Surface'] as const) {
		const edit: ScreenEdit = {
			kind: 'InspectEditor',
			query: { editor, selected: null, document: null, locked: null, retained: null, surface: null }
		};
		assert.equal(screenOperationActivity(edit), 'background');
		let activity: string | undefined;
		await invokeScreenEdit(
			{
				invokeAppOperation: async (_request, _applied, options) => {
					activity = options?.activity;
					return { kind: 'Editor', page: null };
				}
			},
			edit
		);
		assert.equal(activity, 'background');
	}
});

test('authoring still retains visible user activity', () => {
	assert.equal(screenOperationActivity({ kind: 'CreateScreen', label: null }), 'user');
});
