import { describe, expect, it } from 'vitest';

import {
	StatechartDocumentView,
	type StatechartPresentationSnapshot
} from '../statechart-document';

describe('StatechartDocumentView', () => {
	it('keeps identity for an unchanged statechart snapshot and revisions changed snapshots', () => {
		const view = new StatechartDocumentView();
		const states: StatechartPresentationSnapshot['states'] = [];
		const transitions: StatechartPresentationSnapshot['transitions'] = [];
		const first = view.update({ states, transitions });
		const unchanged = view.update({ states, transitions });
		const reallocated = view.update({ states: [], transitions });
		const changed = view.update({
			states: [{ id: 'state', label: 'State', position: { x: 0, y: 0 }, inputs: [], outputs: [] }],
			transitions
		});

		expect(unchanged).toBe(first);
		expect(reallocated).toBe(first);
		expect(changed.revision.sequence).toBe(first.revision.sequence + 1);
	});
});
