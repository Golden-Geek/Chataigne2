import assert from 'node:assert/strict';
import { test } from 'vitest';
import { viewportContour, viewportContourAppearance } from '../contour';

function fixture() {
	const node = Object.assign(new EventTarget(), {
		ownerDocument: { defaultView: new EventTarget() },
		namespaceURI: 'http://www.w3.org/2000/svg',
		style: {} as CSSStyleDeclaration,
		dataset: {} as DOMStringMap
	});
	const changes: boolean[] = [];
	const options = { identity: 'one', onhover: (value: boolean) => changes.push(value) };
	const action = viewportContour(node as unknown as SVGElement, options);
	const send = (type: string, buttons = 0, pointerType = 'mouse') => {
		node.dispatchEvent(Object.assign(new Event(type), { buttons, pointerType }));
	};
	return { node, changes, action, send, options };
}

test('hover paints only a contour without filling or selecting; selected and invalid cues retain precedence', () => {
	const idle = viewportContourAppearance();
	const hover = viewportContourAppearance({ hovered: true });
	assert.equal(hover.fill, idle.fill);
	assert.notEqual(hover.stroke, idle.stroke);
	assert.deepEqual(
		viewportContourAppearance({ selected: true, hovered: true }),
		viewportContourAppearance({ selected: true })
	);
	assert.deepEqual(
		viewportContourAppearance({ valid: false, hovered: true }),
		viewportContourAppearance({ valid: false })
	);
	assert.deepEqual(viewportContourAppearance({ enabled: false, hovered: true }).dash, [0.3, 0.2]);
});

test('non-bubbling native enter/leave events share the same DOM hover lifecycle', () => {
	const f = fixture();
	f.send('pointerenter');
	assert.equal(f.node.dataset.viewportHovered, 'true');
	assert.equal(f.node.style.stroke, '#c0d8ff');
	assert.equal(f.node.style.fill, '#00000000');
	f.send('pointerenter');
	f.send('pointerleave');
	assert.deepEqual(f.changes, [true, false]);
	assert.equal(f.node.style.stroke, '#a2b0c5');
	f.action.destroy();
});

test('touch and captured button movement cannot introduce hover; press, cancel, capture loss and blur retire it', () => {
	const f = fixture();
	f.send('pointerenter', 1);
	f.send('pointerenter', 0, 'touch');
	assert.deepEqual(f.changes, []);
	for (const terminal of [
		'pointerdown',
		'pointercancel',
		'lostpointercapture',
		'pointerleave',
		'blur'
	]) {
		f.send('pointerenter');
		if (terminal === 'blur') f.node.ownerDocument.defaultView.dispatchEvent(new Event('blur'));
		else f.send(terminal);
		assert.equal(f.node.dataset.viewportHovered, 'false', terminal);
	}
	f.action.destroy();
});

test('owner replacement and disposal clear hovered native declarations, with no listeners left behind', () => {
	const f = fixture();
	f.send('pointerenter');
	f.action.update({ ...f.options, identity: 'two' });
	assert.deepEqual(f.changes, [true, false]);
	f.send('pointerenter');
	f.action.destroy();
	assert.deepEqual(f.changes, [true, false, true, false]);
	f.send('pointerenter');
	assert.equal(f.node.dataset.viewportHovered, 'false');
});
