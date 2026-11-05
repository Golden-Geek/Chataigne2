import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createSpatialPointer } from '../pointer.ts';

function fixture() {
	const captures = new Set<number>(),
		moves: { x: number; y: number }[] = [],
		ghosts: unknown[] = [];
	let finished = 0,
		stopped = 0;
	const rect = { left: 10, top: 20, width: 200, height: 100 };
	const canvas = {
		getBoundingClientRect: () => rect,
		setPointerCapture: (id: number) => captures.add(id),
		hasPointerCapture: (id: number) => captures.has(id),
		releasePointerCapture: (id: number) => captures.delete(id)
	};
	const pointer = createSpatialPointer({
		canvas: () => canvas as unknown as SVGSVGElement,
		extent: () => ({ width: 400, height: 200 }),
		ghost: (point) => ghosts.push(point),
		move: (point) => moves.push(point),
		finish: () => finished++,
		stop: () => stopped++
	});
	const event = (x: number, y: number, id = 1, button = 0) =>
		({
			clientX: x,
			clientY: y,
			pointerId: id,
			button,
			preventDefault() {},
			stopPropagation() {}
		}) as PointerEvent;
	return {
		pointer,
		event,
		rect,
		captures,
		moves,
		ghosts,
		get finished() {
			return finished;
		},
		get stopped() {
			return stopped;
		}
	};
}

test('shared Composition/Matrix pointer captures one identity, maps current canvas coordinates and sends the final sample', () => {
	const f = fixture();
	assert(
		f.pointer.start(f.event(30, 40), (point) => {
			assert.deepEqual(point, { x: 40, y: 40 });
			return true;
		})
	);
	assert(f.captures.has(1));
	assert(!f.pointer.start(f.event(40, 40, 2), () => true));
	f.pointer.move(f.event(40, 50, 2));
	assert.equal(f.moves.length, 0);
	f.pointer.move(f.event(50, 60));
	assert.deepEqual(f.moves, [{ x: 80, y: 80 }]);
	f.pointer.release(f.event(60, 70));
	assert.deepEqual(f.moves.at(-1), { x: 100, y: 100 });
	assert.equal(f.finished, 1);
	assert.equal(f.pointer.active, false);
	assert.equal(f.captures.size, 0);
	assert.equal(f.ghosts.at(-1), null);
	f.pointer.cancel();
	assert.equal(f.stopped, 0);
});

test('shared pointer rejects invalid starts and retires capture once on cancellation or a lost viewport', () => {
	for (const lostViewport of [false, true]) {
		const f = fixture();
		assert(!f.pointer.start(f.event(30, 40, 1, 1), () => true));
		assert(!f.pointer.start(f.event(30, 40), () => false));
		assert(f.pointer.start(f.event(30, 40), () => true));
		if (lostViewport) {
			f.rect.width = 0;
			f.pointer.move(f.event(40, 40));
		} else f.pointer.release(f.event(40, 40), true);
		f.pointer.cancel();
		assert.equal(f.stopped, 1);
		assert.equal(f.finished, 0);
		assert.equal(f.captures.size, 0);
		assert.equal(f.ghosts.at(-1), null);
	}
});
