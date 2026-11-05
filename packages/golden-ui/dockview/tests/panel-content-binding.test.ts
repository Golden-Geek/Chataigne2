import { describe, expect, it } from 'vitest';
import { PanelContentBindings } from '../panel-content-binding';

describe('per-instance content binding capability', () => {
	it('publishes only the owning panel and retires callbacks with its content', () => {
		const bindings = new PanelContentBindings();
		const observed: unknown[] = [];
		const dispose = bindings.subscribe('a', (value) => observed.push(value));
		const first = { locked: false, canLock: true, toggle: () => {} };
		const second = { ...first, locked: true };
		bindings.set('b', second);
		bindings.set('a', first);
		bindings.set('a', second);
		bindings.set('a', null);
		expect(observed).toEqual([null, first, second, null]);
		dispose();
		bindings.set('a', first);
		expect(observed).toHaveLength(4);
		const restored: unknown[] = [];
		bindings.subscribe('b', (value) => restored.push(value));
		expect(restored).toEqual([second]);
	});
});
