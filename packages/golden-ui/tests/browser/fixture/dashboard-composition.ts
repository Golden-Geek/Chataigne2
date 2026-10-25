import type { UiNodeDto, ParamValue } from '../../../types';
import type { DashboardLayoutKind } from '../../../components/panels/dashboard/dashboard-model';
import { dashboardData } from './dashboard-data';
import { fixtureNode } from './session';

export function dashboardComposition(
	family: 'generic' | 'node' | 'container',
	parentLayout: DashboardLayoutKind,
	destinationLayout: DashboardLayoutKind
) {
	const data = dashboardData('free', 600, 'dense', true);
	const byId = new Map(data.nodes.map((node) => [node.node_id, node]));
	let next = data.nodes.reduce((max, node) => Math.max(max, node.node_id + 1), 0);
	const clone = (node: UiNodeDto) => ({
		...structuredClone(node),
		node_id: next,
		uuid: fixtureNode(next++, '').uuid
	});
	const fields = (owner: UiNodeDto) =>
		owner.children.map((id) => byId.get(id)!).filter((node) => node.user_role !== 'itemRoot');
	const write = (owner: UiNodeDto, key: string, value: ParamValue) => {
		const field = fields(owner).find((node) => node.decl_id === key)!;
		if (field.data.kind !== 'parameter') throw new Error('Missing composition parameter');
		field.data = { kind: 'parameter', param: { ...field.data.param, value, default_value: value } };
	};
	const append = (node: UiNodeDto) => {
		data.nodes.push(node);
		byId.set(node.node_id, node);
		return node;
	};
	const sourceContainer = byId.get(data.containerId!)!;
	const destination = clone(sourceContainer);
	destination.meta.label = 'Destination';
	destination.children = fields(sourceContainer).map((node) => {
		const copy = clone(node);
		append(copy);
		return copy.node_id;
	});
	append(destination);
	data.page.children.push(destination.node_id);
	write(destination, 'position', { kind: 'vec2', value: [860, 32] });
	write(destination, 'width', { kind: 'css_value', value: 20, unit: 'rem' });
	write(destination, 'height', { kind: 'css_value', value: 25, unit: 'rem' });
	write(destination, 'layout_kind', { kind: 'enum', value: destinationLayout });
	const anchors: number[] = [];
	for (let index = 0; index < 3; index++) {
		const child = clone(byId.get(data.ids[595 + index])!);
		child.meta.label = `Anchor ${index}`;
		child.children = child.children.map((id) => {
			const field = clone(byId.get(id)!);
			append(field);
			return field.node_id;
		});
		append(child);
		write(child, 'position', { kind: 'vec2', value: [8, index * 88 + 8] });
		destination.children.push(child.node_id);
		anchors.push(child.node_id);
	}
	const source = family === 'container' ? sourceContainer : byId.get(data.ids[0])!;
	const parent = family === 'container' ? data.page : sourceContainer;
	write(parent, 'layout_kind', { kind: 'enum', value: parentLayout });
	if (family === 'node') {
		source.node_type = 'dashboard_node_widget';
		source.data = { kind: 'node', node_type: source.node_type };
		const type = fields(source).find((node) => node.decl_id === 'widget_kind')!;
		type.decl_id = 'widget_type';
		if (type.data.kind !== 'parameter') throw new Error('Expected widget type');
		type.data = {
			kind: 'parameter',
			param: {
				...type.data.param,
				value: { kind: 'enum', value: 'fixture' },
				default_value: { kind: 'enum', value: 'fixture' }
			}
		};
		fields(source).find((node) => node.decl_id === 'target_param')!.decl_id = 'target_node';
	}
	const layout = fields(source).filter((node) =>
		['position', 'width', 'height', 'anchor', 'layout_kind'].includes(node.decl_id)
	);
	const folder = fixtureNode(
		next++,
		'Layout',
		layout.map((node) => node.node_id),
		'folder'
	);
	folder.decl_id = 'layout';
	append(folder);
	const moved = new Set(folder.children);
	source.children = [...source.children.filter((id) => !moved.has(id)), folder.node_id];
	for (const field of layout) field.decl_id = `layout/${field.decl_id}`;
	if (new Set(data.nodes.map((node) => node.uuid)).size !== data.nodes.length)
		throw new Error('Composition fixture must have unique node identities');
	return {
		...data,
		source: source.node_id,
		parent: parent.node_id,
		destination: destination.node_id,
		anchors,
		sourceIndex: family === 'container' ? -2 : 0
	};
}
