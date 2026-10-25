import type { ParamValue, UiNodeDto } from '../../../types';
import type { DashboardLayoutKind } from '../../../components/panels/dashboard/dashboard-model';
import { dashboardData } from './dashboard-data';

export function dashboardHierarchy(target: number, kind: 'free' | 'mixed' | 'deep') {
	const count = kind === 'deep' ? 255 : 32;
	const widgets = Math.max(kind === 'deep' ? 768 : 192, Math.ceil((target - 11 - count * 5) / 11));
	const data = dashboardData('free', widgets, 'dense', true);
	const byId = new Map(data.nodes.map((node) => [node.node_id, node]));
	const template = byId.get(data.containerId!)!;
	const metadata = template.children
		.map((id) => byId.get(id)!)
		.filter((node) => node.user_role !== 'itemRoot');
	let nextId = data.nodes.reduce((maximum, node) => Math.max(maximum, node.node_id), 0) + 1;
	const families: DashboardLayoutKind[] = [
		'free',
		'horizontal',
		'vertical',
		'grid',
		'tabs',
		'accordion'
	];
	const containers: { id: number; layout: DashboardLayoutKind; ids: number[] }[] = [];
	const objects: UiNodeDto[] = [];
	const write = (fields: UiNodeDto[], key: string, value: ParamValue) => {
		const field = fields.find((node) => node.decl_id === key)!;
		if (field.data.kind !== 'parameter') throw new Error('Expected container parameter');
		field.data = { kind: 'parameter', param: { ...field.data.param, value, default_value: value } };
	};
	data.page.children = data.page.children.filter((id) => id !== template.node_id);
	for (let index = 0; index < count; index++) {
		const container = index === 0 ? template : structuredClone(template);
		const fields =
			index === 0
				? metadata
				: metadata.map((field) => {
						const clone = structuredClone(field);
						clone.node_id = nextId++;
						data.nodes.push(clone);
						return clone;
					});
		if (index > 0) {
			container.node_id = nextId++;
			data.nodes.push(container);
		}
		const layout = kind === 'mixed' ? families[index % families.length] : 'free';
		container.meta.label = `Container ${index} ${layout}`;
		container.children = fields.map((field) => field.node_id);
		write(fields, 'position', {
			kind: 'vec2',
			value: kind === 'deep' ? [0, 0] : [(index % 8) * 140, Math.floor(index / 8) * 195]
		});
		write(fields, 'width', { kind: 'css_value', value: kind === 'deep' ? 50 : 8, unit: 'rem' });
		write(fields, 'height', { kind: 'css_value', value: kind === 'deep' ? 40 : 10, unit: 'rem' });
		write(fields, 'layout_kind', { kind: 'enum', value: layout });
		containers.push({ id: container.node_id, layout, ids: [] });
		objects.push(container);
		if (kind === 'deep' && index > 0)
			objects[Math.floor((index - 1) / 2)].children.push(container.node_id);
		else data.page.children.push(container.node_id);
	}
	for (let index = 0; index < widgets; index++) {
		const parent = kind === 'deep' ? 127 + (index % 128) : index % count;
		objects[parent].children.push(data.ids[index]);
		containers[parent].ids.push(data.ids[index]);
	}
	return { ...data, containers };
}
