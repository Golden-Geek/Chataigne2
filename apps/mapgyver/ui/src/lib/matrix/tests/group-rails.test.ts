import { test } from 'node:test';
import assert from 'node:assert/strict';
import { groupRail, rowInset } from '../group-rails.ts';
import { projectGrid, gridSize } from '../viewport.ts';
import type { MatrixGroupSpan } from '../../media/generated/MatrixGroupSpan';

const span: MatrixGroupSpan = {
	id: 'outer',
	depth: 0,
	start: 0,
	end: 3,
	start_groups: 0,
	end_groups: 2
};
const metrics = { top: 0, left: 0, width: 900, height: 500, rem: 16 };

test('nested rails use mixed-height Group intervals and distinct restrained gutters', () => {
	const view = projectGrid(metrics, 3, 4, 0, 2);
	const outer = groupRail(span, view, metrics.height / metrics.rem, 1)!;
	const inner = groupRail(
		{ ...span, id: 'inner', depth: 1, start: 1, start_groups: 1 },
		view,
		metrics.height / metrics.rem,
		1
	)!;
	assert.equal(outer.top, view.header + gridSize.group - 0.1);
	assert.equal(outer.bottom, view.header + view.rowExtent - 0.08);
	assert(inner.x > outer.x);
	assert(inner.top > outer.top);
	assert.equal(inner.bottom, outer.bottom);
	assert(!outer.startClipped && !outer.endClipped);
});

test('an offscreen Group header retains a clipped rail across a middle virtual window', () => {
	const view = projectGrid({ ...metrics, top: 400 }, 10002, 100, 0, 2);
	const rail = groupRail({ ...span, end: 10002 }, view, metrics.height / metrics.rem, 1)!;
	assert.equal(rail.top, view.header);
	assert.equal(rail.bottom, metrics.height / metrics.rem);
	assert(rail.startClipped && rail.endClipped);
	assert(
		!rail.path.includes('Q'),
		'Clipped endpoints do not pretend the Group begins or ends here'
	);
	assert(!rail.path.includes('NaN') && !rail.path.includes('Infinity'));
});

test('collapsed/empty Group rails stay within the compact header and vanished ranges paint nothing', () => {
	const view = projectGrid(metrics, 4, 2, 0, 2);
	const rail = groupRail({ ...span, end: 1, end_groups: 1 }, view, 100, 0)!;
	assert.equal(rail.top, view.header + 0.35);
	assert.equal(rail.bottom, view.header + gridSize.group - 0.3);
	assert.equal(groupRail({ ...span, start: 1000, end: 1001, start_groups: 2 }, view, 20, 0), null);
});

test('every admitted nesting level has a distinct rail inside the label gutter at every font scale', () => {
	for (const rem of [12, 16, 24, 32]) {
		const view = projectGrid({ ...metrics, rem }, 100, 10, 0, 32);
		const rails = Array.from({ length: 32 }, (_, depth) =>
			groupRail({ ...span, depth, end: 100, end_groups: 32 }, view, 1000, 31)!
		);
		const positions = rails.map((rail) => rail.x);
		assert.equal(new Set(positions).size, 32);
		assert(positions.at(-1)! < gridSize.label * 0.4 + 0.4);
		for (const [depth, rail] of rails.entries())
			assert(
				rail.x + 0.04 < rowInset(depth + 1),
				'Ancestor wrap remains outside the entire child selection contour, even at maximum nesting'
			);
	}
});
