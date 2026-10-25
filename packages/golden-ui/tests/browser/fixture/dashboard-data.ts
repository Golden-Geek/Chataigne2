import type { ParamValue, UiNodeDto } from '../../../types';
import type { DashboardLayoutKind } from '../../../components/panels/dashboard/dashboard-model';
import { fixtureNode } from './session';

export function dashboardData(layout: DashboardLayoutKind, sparseCount = 0) {
	const nodes: UiNodeDto[] = [];
	let nextId = 1;
	const add = (type: string, label: string, item = true) => {
		const node = fixtureNode(nextId++, label, [], type);
		if (item) node.user_role = 'itemRoot';
		nodes.push(node);
		return node;
	};
	const param = (owner: UiNodeDto, key: string, value: ParamValue, enabled = true) => {
		const node = add('parameter', key, false);
		node.decl_id = key;
		node.meta.enabled = enabled;
		node.data = {
			kind: 'parameter',
			param: {
				value,
				default_value: value,
				event_behaviour: 'Coalesce',
				read_only: false,
				constraints: { enum_options: [], policy: 'ClampAdapt' },
				ui_hints: {},
				control: { mode: 'manual', spec: { mode: 'manual' } },
				reference_allowed_targets: [],
				reference_visible_nodes: []
			}
		};
		owner.children.push(node.node_id);
		return node;
	};
	const enumParam = (owner: UiNodeDto, key: string, value: string) =>
		param(owner, key, { kind: 'enum', value });
	const cssParam = (owner: UiNodeDto, key: string, value: number) =>
		param(owner, key, { kind: 'css_value', value, unit: 'rem' });
	const place = (node: UiNodeDto, x: number, y: number, width = 12, height = 5) => {
		param(node, 'position', { kind: 'vec2', value: [x, y] });
		cssParam(node, 'width', width);
		cssParam(node, 'height', height);
	};
	const root = fixtureNode(0, 'Project', [], 'FixtureRoot');
	nodes.push(root);
	const page = add('dashboard_page', 'Dashboard');
	root.children.push(page.node_id);
	enumParam(page, 'layout_kind', layout);
	param(page, 'layout/page_size', { kind: 'vec2', value: [1200, 850] });
	cssParam(page, 'gap_x', 0.7);
	cssParam(page, 'gap_y', 0.9);
	param(page, 'snap_grid', { kind: 'float', value: 0.5 });
	param(page, 'grid_line_count', { kind: 'int', value: 3 });
	const reference = (target: UiNodeDto): ParamValue => ({
		kind: 'reference',
		uuid: target.uuid,
		cached_id: target.node_id
	});
	const target = param(root, 'Target', { kind: 'float', value: 0.42 });
	const checkbox = param(root, 'Enabled', { kind: 'bool', value: true });
	const text = param(root, 'Message', { kind: 'str', value: 'Editable value' });
	const ids: number[] = [];
	for (let index = 0; index < (sparseCount || 6); index++) {
		const kinds = ['text', 'button', 'slider', 'checkbox', 'textInput', 'textInput'];
		const widget = add('dashboard_generic_widget', `Widget ${index}`);
		ids.push(widget.node_id);
		page.children.push(widget.node_id);
		place(
			widget,
			sparseCount ? index * 20 : (index % 3) * 16 + 2,
			sparseCount ? 2 : Math.floor(index / 3) * 10 + 2
		);
		enumParam(widget, 'anchor', 'top-left');
		enumParam(widget, 'widget_kind', kinds[index % kinds.length]);
		enumParam(widget, 'label_placement', ['left', 'right', 'top', 'bottom', 'inside'][index % 5]);
		param(widget, 'text', { kind: 'str', value: `Content ${index}` });
		param(widget, 'target_param', reference(index === 3 ? checkbox : index >= 4 ? text : target));
		param(widget, 'value_range', { kind: 'vec2', value: [0, 1] });
		param(widget, 'multiline', { kind: 'bool', value: index === 5 });
	}
	if (!sparseCount) {
		const container = add('dashboard_widget_container', 'Nested container');
		page.children.push(container.node_id);
		place(container, 2, 25, 40, 18);
		enumParam(container, 'layout_kind', 'horizontal');
		for (const anchor of ['top-left', 'center', 'bottom-right']) {
			const widget = add('dashboard_generic_widget', anchor);
			container.children.push(widget.node_id);
			place(widget, 1, 1, 10, 5);
			enumParam(widget, 'anchor', anchor);
			param(widget, 'text', { kind: 'str', value: anchor });
		}
		const registered = add('dashboard_node_widget', 'Registered widget');
		page.children.push(registered.node_id);
		place(registered, 45, 25, 20, 8);
		enumParam(registered, 'widget_type', 'fixture');
		enumParam(registered, 'label_placement', 'inside');
		param(registered, 'target_node', reference(target));
	}
	return { nodes, page, ids };
}
