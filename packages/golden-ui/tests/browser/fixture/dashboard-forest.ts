import type { ParamValue, UiNodeDto } from '../../../types';
import { dashboardData } from './dashboard-data';

const setValue = (node: UiNodeDto, value: ParamValue) => {
	if (node.data.kind !== 'parameter') throw new Error('Expected fixture parameter');
	node.data = { kind: 'parameter', param: { ...node.data.param, value, default_value: value } };
};

export function dashboardForest(count: number) {
	const data = dashboardData('free', count, 'grid', true);
	const byId = new Map(data.nodes.map((node) => [node.node_id, node]));
	const template = byId.get(data.containerId!)!;
	const metadata = template.children
		.map((id) => byId.get(id)!)
		.filter((node) => node.user_role !== 'itemRoot');
	let nextId = Math.max(...data.nodes.map((node) => node.node_id)) + 1;
	const containers: number[] = [];
	for (let index = 0; index < count; index++) {
		const container = index === 0 ? template : structuredClone(template);
		const fields =
			index === 0
				? metadata
				: metadata.map((node) => {
						const clone = structuredClone(node);
						clone.node_id = nextId++;
						data.nodes.push(clone);
						return clone;
					});
		if (index > 0) {
			container.node_id = nextId++;
			data.nodes.push(container);
			data.page.children.push(container.node_id);
		}
		setValue(
			fields.find((field) => field.decl_id === 'position')!,
			{ kind: 'vec2', value: [6000 + index * 1000, 6000] }
		);
		container.children = [...fields.map((node) => node.node_id), data.ids[index]];
		const child = byId.get(data.ids[index])!;
		const position = child.children
			.map((id) => byId.get(id)!)
			.find((node) => node.decl_id === 'position')!;
		setValue(position, { kind: 'vec2', value: [0, 0] });
		containers.push(container.node_id);
	}
	return { ...data, containers };
}
