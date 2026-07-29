import { afterEach, describe, expect, it } from 'vitest';
import {
	clearCustomNodeInspectors,
	registerNodeInspector,
	registerNodeInspectorMatcher,
	resolveNodeInspector,
	type UiNodeDto
} from 'golden_ui';

afterEach(() => {
	clearCustomNodeInspectors();
});

describe('node inspector registry', () => {
	it('resolves an app-specific inspector from a matcher', () => {
		const matchedComponent = {};
		const node = {
			node_type: 'folder',
			user_item_kind: '',
			decl_id: 'connection'
		} as UiNodeDto;

		registerNodeInspectorMatcher(
			'sound-card-connection',
			(candidate) => candidate.decl_id === 'connection',
			{ component: matchedComponent }
		);

		expect(resolveNodeInspector(node)?.component).toBe(matchedComponent);
	});

	it('keeps an explicit node-type registration ahead of matcher registrations', () => {
		const typeComponent = {};
		const matchedComponent = {};
		const node = {
			node_type: 'folder',
			user_item_kind: '',
			decl_id: 'connection'
		} as UiNodeDto;

		registerNodeInspector('folder', { component: typeComponent });
		registerNodeInspectorMatcher('connection', () => true, {
			component: matchedComponent
		});

		expect(resolveNodeInspector(node)?.component).toBe(typeComponent);
	});
});
