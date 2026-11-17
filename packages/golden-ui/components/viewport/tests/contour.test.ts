import assert from 'node:assert/strict';
import { test } from 'vitest';
import { viewportContour, viewportContourAppearance } from '../contour';

function fixture(svg = true) {
	const node = Object.assign(new EventTarget(), {
		ownerDocument: { defaultView: new EventTarget() },
		namespaceURI: svg ? 'http://www.w3.org/2000/svg' : 'http://www.w3.org/1999/xhtml',
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

test('DOM contours expose theme fallbacks without changing state precedence or border geometry', () => {
	const f = fixture(false);
	assert.equal(f.node.style.borderColor, 'var(--gc-viewport-contour-idle, #a2b0c5)');
	assert.equal(f.node.style.borderWidth, '0.0625rem');
	f.send('pointerenter');
	assert.equal(f.node.style.borderColor, 'var(--gc-viewport-contour-hover, #c0d8ff)');
	f.action.update({ ...f.options, selected: true });
	assert.equal(f.node.style.borderColor, 'var(--gc-viewport-contour-selected, #70aaff)');
	f.action.update({ ...f.options, selected: true, valid: false, enabled: false });
	assert.equal(f.node.style.borderColor, 'var(--gc-viewport-contour-invalid, #ff5364)');
	assert.equal(f.node.style.borderStyle, 'dashed');
	f.action.destroy();
});

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

test('keyboard focus shares contour emphasis without becoming hover or selecting and clears on identity replacement', () => {
	const f = fixture();
	const focus: boolean[] = [];
	f.action.update({ ...f.options, onfocus: (value) => focus.push(value) });
	f.send('focus');
	assert.equal(f.node.dataset.viewportFocused, 'true');
	assert.equal(f.node.dataset.viewportHovered, 'false');
	assert.equal(f.node.style.stroke, '#c0d8ff');
	assert.deepEqual(f.changes, []);
	f.action.update({ ...f.options, identity: 'two', onfocus: (value) => focus.push(value) });
	assert.deepEqual(focus, [true, false]);
	f.action.destroy();
	f.send('focus');
	assert.equal(f.node.dataset.viewportFocused, 'false');
});
