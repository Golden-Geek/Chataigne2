import { expect, test } from 'vitest';
import { canvasNavigationCommand } from '../canvas-shortcuts';

const key = (key: string, flags = {}) => ({
	key,
	defaultPrevented: false,
	ctrlKey: false,
	metaKey: false,
	altKey: false,
	...flags
});
test('canvas navigation shortcuts share command mapping and preserve modified/domain-consumed keys', () => {
	for (const [value, command] of [
		['f', 'frame'],
		['H', 'home'],
		['Home', 'home'],
		['+', 'zoom-in'],
		['=', 'zoom-in'],
		['-', 'zoom-out']
	])
		expect(canvasNavigationCommand(key(value))).toBe(command);
	for (const flags of [
		{ ctrlKey: true },
		{ metaKey: true },
		{ altKey: true },
		{ defaultPrevented: true }
	])
		expect(canvasNavigationCommand(key('f', flags))).toBeNull();
	expect(canvasNavigationCommand(key('ArrowLeft'))).toBeNull();
});
