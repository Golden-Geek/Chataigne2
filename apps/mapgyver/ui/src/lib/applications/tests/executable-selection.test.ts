import assert from 'node:assert/strict';
import { test } from 'node:test';
import { ExecutableSelection, type ExecutableSelectionSnapshot } from '../executable-selection.ts';

function deferred() {
	let resolve!: (path: string | null) => void;
	let reject!: (error: Error) => void;
	const promise = new Promise<string | null>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
}

test('selection changes only the form draft and cancellation preserves exact manually entered paths', async () => {
	let choice: string | null = null;
	const snapshots: ExecutableSelectionSnapshot[] = [];
	const selection = new ExecutableSelection(
		'D:\\Apps\\old.exe',
		async () => choice,
		() => true,
		(snapshot) => snapshots.push(snapshot)
	);
	selection.setPath('D:\\A B\\日本語.exe');
	await selection.browse();
	assert.deepEqual(snapshots.at(-1), { path: 'D:\\A B\\日本語.exe', browsing: false, error: null });
	choice = 'D:\\New App\\chosen.exe';
	await selection.browse();
	assert.deepEqual(snapshots.at(-1), { path: choice, browsing: false, error: null });
	assert.equal(
		snapshots[0].path,
		'D:\\A B\\日本語.exe',
		'published snapshots must remain immutable'
	);
	selection.setPath('');
	assert.equal(snapshots.at(-1)?.path, '');
});

test('one pending dialog blocks duplicate browsing and editing; disabling discards the late choice', async () => {
	const dialog = deferred();
	let enabled = true,
		requests = 0;
	const snapshots: ExecutableSelectionSnapshot[] = [];
	const selection = new ExecutableSelection(
		'old.exe',
		() => {
			requests++;
			return dialog.promise;
		},
		() => enabled,
		(snapshot) => snapshots.push(snapshot)
	);
	const pending = selection.browse();
	await selection.browse();
	selection.setPath('conflicting.exe');
	assert.equal(requests, 1);
	assert.equal(snapshots.length, 1);
	assert.equal(snapshots[0].browsing, true);
	enabled = false;
	dialog.resolve('late.exe');
	await pending;
	assert.deepEqual(snapshots.at(-1), { path: 'old.exe', browsing: false, error: null });
	await selection.browse();
	selection.setPath('blocked.exe');
	assert.equal(requests, 1);
	assert.equal(snapshots.at(-1)?.path, 'old.exe');
});

test('destroyed forms cannot publish late choices or failures into a newly selected application', async () => {
	for (const rejected of [false, true]) {
		const dialog = deferred();
		const old: ExecutableSelectionSnapshot[] = [],
			current: ExecutableSelectionSnapshot[] = [];
		const selection = new ExecutableSelection(
			'first.exe',
			() => dialog.promise,
			() => true,
			(snapshot) => old.push(snapshot)
		);
		const pending = selection.browse();
		selection.dispose();
		const next = new ExecutableSelection(
			'second.exe',
			async () => 'second-choice.exe',
			() => true,
			(snapshot) => current.push(snapshot)
		);
		await next.browse();
		if (rejected) dialog.reject(new Error('old dialog failed'));
		else dialog.resolve('wrong-application.exe');
		await pending;
		selection.setPath('disposed.exe');
		await selection.browse();
		assert.equal(old.length, 1);
		assert.deepEqual(current.at(-1), { path: 'second-choice.exe', browsing: false, error: null });
	}
});

test('dialog failures preserve the path, end pending state and permit retry without authoring effects', async () => {
	let fail = true;
	const snapshots: ExecutableSelectionSnapshot[] = [];
	const selection = new ExecutableSelection(
		'unchanged.exe',
		async () => {
			if (fail) throw new Error('dialog unavailable');
			return 'retry.exe';
		},
		() => true,
		(snapshot) => snapshots.push(snapshot)
	);
	await selection.browse();
	assert.deepEqual(snapshots.at(-1), {
		path: 'unchanged.exe',
		browsing: false,
		error: 'Error: dialog unavailable'
	});
	fail = false;
	await selection.browse();
	assert.deepEqual(snapshots.at(-1), { path: 'retry.exe', browsing: false, error: null });
});
