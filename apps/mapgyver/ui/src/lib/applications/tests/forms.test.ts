import { test } from 'node:test';
import assert from 'node:assert/strict';
import { definitionFromForm } from '../definition-form.ts';
import { associateApplication, columnDistances, removeAssociation } from '../column-form.ts';
import type { ApplicationDefinition } from '../../media/generated/ApplicationDefinition';
import type { ColumnSnapshot } from '../../media/generated/ColumnSnapshot';

const definition: ApplicationDefinition = {
	executable: null,
	arguments: [],
	directory: null,
	environment: null,
	window: 'Hidden',
	readiness: { kind: 'ProcessStarted' },
	stop: { graceful_ms: 2000, retirement_ms: 5000 },
	restart: { enabled: true, initial_ms: 500, maximum_ms: 30000 }
};
function data(entries: Record<string, string>) {
	const data = new FormData();
	for (const [key, value] of Object.entries(entries)) data.set(key, value);
	return data;
}
function fields() {
	return data({
		executable: 'D:\\Apps\\Renderer.exe',
		directory: 'D:\\My Media',
		window: 'Normal',
		'graceful-stop': '30',
		'retirement-timeout': '1000',
		'restart-initial': '100',
		'restart-maximum': '2000',
		restart: 'on',
		'inherit-environment': 'on'
	});
}

test('definition form preserves exact argument boundaries and inherited versus empty replacement environments', () => {
	const form = fields();
	for (const argument of ['--file', 'D:\\a b\\scene.json', '"quoted"', ''])
		form.append('argument', argument);
	const result = definitionFromForm(form, definition, 'ProcessStarted', '');
	assert.deepEqual(result.arguments, ['--file', 'D:\\a b\\scene.json', '"quoted"', '']);
	assert.equal(result.environment, null);
	assert.equal(result.directory, 'D:\\My Media');
	assert.equal(result.window, 'Normal');
	form.delete('inherit-environment');
	assert.deepEqual(definitionFromForm(form, definition, 'ProcessStarted', '').environment, []);
	form.append('environment-name', 'EXPLICIT');
	form.append('environment-value', '');
	assert.deepEqual(definitionFromForm(form, definition, 'ProcessStarted', '').environment, [
		['EXPLICIT', '']
	]);
	assert.equal(definition.executable, null);
});

test('Spout readiness requires explicit evidence and timeout, with no UI-selected defaults', () => {
	const form = fields();
	assert.throws(() => definitionFromForm(form, definition, 'Spout', 'renderer'));
	form.set('readiness-evidence', 'image');
	form.set('readiness-timeout', '3500');
	assert.deepEqual(definitionFromForm(form, definition, 'Spout', 'renderer').readiness, {
		kind: 'Spout',
		sender: 'renderer',
		first_frame: true,
		timeout_ms: 3500
	});
	form.set('readiness-evidence', 'sender');
	assert.equal(definitionFromForm(form, definition, 'Spout', 'renderer').readiness.kind, 'Spout');
	form.set('readiness-timeout', '');
	assert.throws(() => definitionFromForm(form, definition, 'Spout', 'renderer'));
});

const column: ColumnSnapshot = {
	id: 'column',
	label: 'Column 3',
	definition: {
		transition: { duration_ms: 250, curve: 'Linear' },
		applications: [
			{ application: 'shared', pre: 1, post: 1 },
			{ application: 'missing', pre: 5, post: 9 }
		]
	}
};
test('column forms preserve transition and saved missing references while sending explicit typed distances', () => {
	const form = data({
		'pre:shared': '2',
		'post:shared': '4',
		'pre:missing': '5',
		'post:missing': '9'
	});
	const edit = columnDistances(column, form);
	assert.equal(edit.kind, 'ConfigureColumn');
	if (edit.kind === 'ConfigureColumn') {
		assert.deepEqual(edit.definition.transition, column.definition.transition);
		assert.deepEqual(edit.definition.applications, [
			{ application: 'shared', pre: 2, post: 4 },
			{ application: 'missing', pre: 5, post: 9 }
		]);
	}
	const add = associateApplication(column, 'another-shared-id', data({ pre: '0', post: '3' }));
	if (add.kind === 'ConfigureColumn')
		assert.deepEqual(add.definition.applications[2], {
			application: 'another-shared-id',
			pre: 0,
			post: 3
		});
	const remove = removeAssociation(column, 'shared');
	if (remove.kind === 'ConfigureColumn')
		assert.deepEqual(remove.definition.applications, [{ application: 'missing', pre: 5, post: 9 }]);
	assert.throws(() => associateApplication(column, 'new', data({ pre: '', post: '1' })));
	assert.equal(column.definition.applications[0].pre, 1);
});
