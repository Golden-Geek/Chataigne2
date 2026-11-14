import { expect, test } from 'vitest';
import { selectedEditorItem } from '../editor-selection';

test('explicit deselection ignores retained definitions and never selects the first visible item', () => {
	const visible = [{ id: 'a' }, { id: 'b' }];
	for (const selected of [null, undefined, 'missing']) {
		expect(selectedEditorItem(selected, visible, { id: 'old' }, (item) => item.id)).toBeNull();
	}
	expect(visible).toEqual([{ id: 'a' }, { id: 'b' }]);
});

test('a current pointer choice overrides an old selected definition and matching off-page content stays available', () => {
	const visible = [{ id: 'a' }, { id: 'b' }],
		offpage = { id: 'offpage' };
	expect(selectedEditorItem('b', visible, offpage, (item) => item.id)).toBe(visible[1]);
	expect(selectedEditorItem('offpage', visible, offpage, (item) => item.id)).toBe(offpage);
	expect(selectedEditorItem('other', visible, offpage, (item) => item.id)).toBeNull();
});

test('zero is a valid numeric object identity', () => {
	const visible = [{ id: 0 }];
	expect(selectedEditorItem(0, visible, null, (item) => item.id)).toBe(visible[0]);
});
