import assert from 'node:assert/strict';
import { test } from 'node:test';
import { ReadQueue } from '../read-queue.ts';

const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
const unexpectedError = (error: unknown) => assert.fail(String(error));
const deferred = () => {
	let resolve!: (value: number) => void;
	const promise = new Promise<number>((yes) => {
		resolve = yes;
	});
	return { promise, resolve };
};

test('progressive reads publish ordered live observations while retaining one latest replacement', async () => {
	const queue = new ReadQueue(),
		first = deferred(),
		last = deferred();
	const seen: number[] = [];
	let calls = 0;
	const read = () => (++calls === 1 ? first.promise : last.promise);
	for (let i = 0; i < 100; i++)
		queue.refresh(
			['same owner', 1],
			() => true,
			read,
			(value) => seen.push(value),
			unexpectedError,
			() => {}
		);
	assert.equal(calls, 1);
	first.resolve(1);
	await flush();
	assert.deepEqual(
		seen,
		[1],
		'active observation remains visible while input and its next observation continue'
	);
	assert.equal(calls, 2);
	last.resolve(2);
	await flush();
	assert.deepEqual(seen, [1, 2]);
});

test('context replacement and explicit cancellation discard old observations and queued work', async () => {
	for (const invalidate of [false, true]) {
		const queue = new ReadQueue(),
			first = deferred();
		const seen: number[] = [];
		let calls = 0;
		queue.refresh(
			['old'],
			() => true,
			() => {
				calls++;
				return first.promise;
			},
			(value) => seen.push(value),
			unexpectedError,
			() => {}
		);
		queue.refresh(
			['new'],
			() => true,
			async () => {
				calls++;
				return 2;
			},
			(value) => seen.push(value),
			unexpectedError,
			() => {}
		);
		if (invalidate) queue.invalidate();
		first.resolve(1);
		await flush();
		assert.deepEqual(seen, invalidate ? [] : [2]);
		assert.equal(calls, invalidate ? 1 : 2);
	}
});

test('ordinary requests retain latest-only results instead of adopting progressive semantics', async () => {
	const queue = new ReadQueue(),
		first = deferred();
	const seen: number[] = [];
	queue.request(
		() => true,
		() => first.promise,
		(value) => seen.push(value),
		unexpectedError,
		() => {}
	);
	queue.request(
		() => true,
		async () => 2,
		(value) => seen.push(value),
		unexpectedError,
		() => {}
	);
	first.resolve(1);
	await flush();
	assert.deepEqual(seen, [2]);
});
