import { describe, expect, it } from 'vitest';
import { IdentifiedNodeMap } from '../graph-node-index';

describe('projected node UUID lookup', () => {
	it('preserves complete identities when hashes collide, including after a fork', () => {
		// These distinct identities both have FNV-1a hash 1582148253.
		const nodes = new IdentifiedNodeMap([
			[1, { uuid: 'costarring' }],
			[2, { uuid: 'liquid' }]
		]);
		const pending = nodes.fork();
		pending.delete(1);
		expect(pending.getByUuid('costarring')).toBeUndefined();
		expect(pending.getByUuid('liquid')).toEqual({ uuid: 'liquid' });
		expect(nodes.getByUuid('costarring')).toEqual({ uuid: 'costarring' });
	});
	it('keeps published and cancelled identities isolated across forks, replacement and deletion', () => {
		const published = new IdentifiedNodeMap<{ uuid: string; label: string }>();
		published.set(1, { uuid: 'owner', label: 'First' });
		const pending = published.fork();
		pending.delete(1);
		pending.set(5, { uuid: 'owner', label: 'Loaded at another ID' });
		expect(published.getByUuid('owner')?.label).toBe('First');
		expect(pending.getByUuid('owner')?.label).toBe('Loaded at another ID');
		pending.set(5, { uuid: 'replacement', label: 'Reused ID' });
		expect(pending.getByUuid('owner')).toBeUndefined();
		expect(pending.getByUuid('replacement')?.label).toBe('Reused ID');
		published.set(1, { uuid: 'owner', label: 'Renamed' });
		expect(pending.getByUuid('owner')).toBeUndefined();
		pending.clear();
		expect(pending.getByUuid('replacement')).toBeUndefined();
		expect(published.getByUuid('owner')?.label).toBe('Renamed');
	});
});
