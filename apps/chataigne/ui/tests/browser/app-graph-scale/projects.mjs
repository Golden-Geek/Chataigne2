import assert from 'node:assert/strict';
import { randomUUID, createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';

/** Canonical persisted fixtures exported by the owning Rust/native cases, expanded before load. */
export const prototype = (consumer, mode = 'voronoi') => {
	const name = consumer === 'spatializer' && mode !== 'voronoi' ? `${consumer}-${mode}` : consumer;
	return JSON.parse(readFileSync(new URL(`./fixtures/${name}.json`, import.meta.url), 'utf8'));
};
export const fingerprint = (project) =>
	createHash('sha256').update(JSON.stringify(project)).digest('hex');
export function records(root) {
	const rows = [],
		stack = [root];
	while (stack.length) {
		const node = stack.pop();
		rows.push(node);
		for (const child of node.children ?? []) stack.push(child);
	}
	return rows;
}
const find = (root, type) => {
	const result = records(root).find((node) => node.type === type);
	assert.ok(result, `Missing canonical ${type} prototype`);
	return result;
};
const declaration = (node) => node.meta?.decl_id?.split('/').at(-1);
const child = (node, key) => {
	const result = node.children?.find((node) => declaration(node) === key);
	assert.ok(result, `Missing canonical ${key} field`);
	return result;
};
function rewriteUuids(value, replacements) {
	if (!value || typeof value !== 'object') return;
	if (typeof value.uuid === 'string' && replacements.has(value.uuid)) {
		value.uuid = replacements.get(value.uuid);
		// References must resolve the new authored UUID instead of retaining prototype hints.
		delete value.cached_name;
		delete value.relative_path_from_root;
	}
	for (const nested of Object.values(value)) rewriteUuids(nested, replacements);
}
function clone(root) {
	const tree = structuredClone(root),
		replacements = new Map();
	for (const node of records(tree)) {
		const uuid = randomUUID();
		replacements.set(node.uuid, uuid);
		node.uuid = uuid;
	}
	rewriteUuids(tree, replacements);
	return tree;
}
function presentation(node, index, name, spacing = 24) {
	node.meta = {
		...node.meta,
		label: `${name} ${String(index).padStart(6, '0')}`,
		presentation: { ...node.meta?.presentation, collapsed: false }
	};
	const position = child(node, ['Target', 'Source'].includes(name) ? 'position_2d' : 'position');
	assert.equal(position.type, 'vec2', 'Expected canonical Vec2 position declaration');
	position.data = {
		...position.data,
		value: { Vec2: [2 + (index % 256) * spacing, 3 + Math.floor(index / 256) * spacing] }
	};
}
function state(project, count) {
	const owner = find(project.root, 'state_machine_manager');
	const template = owner.children.find((node) => node.type === 'state');
	const target = owner.children.find((node) => node.type === 'state' && node !== template);
	assert.ok(template && target);
	const transition = find(template, 'state_transition');
	assert.equal(
		target.meta.enabled,
		false,
		'State prototype must carry backend-normalized inactivity'
	);
	const nodes = Array.from({ length: count }, (_, index) => clone(index ? target : template));
	for (let index = 0; index < nodes.length; index++) {
		const node = nodes[index];
		presentation(node, index, 'State');
		const transitions = find(node, 'state_transition_manager');
		transitions.children = [];
		if (index !== nodes.length - 1) {
			const edge = clone(transition);
			rewriteUuids(edge, new Map([[target.uuid, nodes[index + 1].uuid]]));
			transitions.children.push(edge);
		}
	}
	owner.children = [...owner.children.filter((node) => node.type !== 'state'), ...nodes];
	rewriteUuids(
		project.root,
		new Map([
			[template.uuid, nodes[0].uuid],
			[target.uuid, nodes.at(-1).uuid]
		])
	);
	return { owner: owner.uuid, items: nodes.map((node) => node.uuid), edges: count - 1 };
}
function formula(project, count) {
	const owner = find(project.root, 'alchemist_formula');
	const constant = owner.children.find((node) => node.type === 'alchemist_anode');
	const math = owner.children.find((node) => node.type === 'alchemist_anode' && node !== constant);
	const edge = find(owner, 'alchemist_connection');
	assert.ok(constant && math);
	const nodes = Array.from({ length: count }, (_, index) => clone(index ? math : constant));
	const edges = [];
	for (let index = 0; index < nodes.length; index++) {
		presentation(nodes[index], index, 'Formula');
		if (!index) continue;
		const connection = clone(edge);
		rewriteUuids(
			connection,
			new Map([
				[constant.uuid, nodes[index - 1].uuid],
				[math.uuid, nodes[index].uuid]
			])
		);
		child(connection, 'source_socket').data.value.Str = index === 1 ? 'value' : 'result';
		child(connection, 'source_socket').data.default_value.Str = index === 1 ? 'value' : 'result';
		edges.push(connection);
	}
	owner.children = [
		...owner.children.filter(
			(node) => !['alchemist_anode', 'alchemist_connection'].includes(node.type)
		),
		...nodes,
		...edges
	];
	return { owner: owner.uuid, items: nodes.map((node) => node.uuid), edges: edges.length };
}
function spatializer(project, count, { sources: sourceCount = 32 } = {}) {
	const owner = find(project.root, 'spatializer_module');
	assert.ok(count <= 4096, 'Spatializer fixture exceeds the backend target limit');
	assert.ok(
		Number.isSafeInteger(sourceCount) && sourceCount > 0 && sourceCount <= 4096,
		'Spatializer fixture exceeds the backend source limit'
	);
	assert.ok(sourceCount * count <= 1048576, 'Spatializer fixture exceeds the backend pair limit');
	const sourceList = find(owner, 'spatializer_source_list');
	const sourceTemplate = find(sourceList, 'spatializer_source');
	const sources = Array.from({ length: sourceCount }, (_, index) => {
		const node = clone(sourceTemplate);
		presentation(node, index, 'Source', 6);
		child(node, 'position_2d').data.value.Vec2[1] = -3 - Math.floor(index / 256) * 6;
		return node;
	});
	sourceList.children = sources;
	const targets = find(owner, 'spatializer_target_list');
	const template = find(targets, 'spatializer_target');
	const nodes = Array.from({ length: count }, () => clone(template));
	for (let index = 0; index < nodes.length; index++) presentation(nodes[index], index, 'Target', 6);
	targets.children = nodes;
	const values = owner.children?.find((node) => declaration(node) === 'values');
	if (values) values.children = [];
	return {
		owner: owner.uuid,
		items: nodes.map((node) => node.uuid),
		sourceItems: sources.map((node) => node.uuid),
		sources: sources.length,
		targets: count,
		pairs: sources.length * count
	};
}
export function expand(consumer, count, options = {}) {
	assert.ok(Number.isSafeInteger(count) && count >= 8);
	const { mode = 'voronoi' } = options;
	const project = prototype(consumer, mode);
	const metadata = {
		consumer,
		...{ state, formula, spatializer }[consumer](project, count, options)
	};
	if (consumer === 'spatializer') metadata.mode = mode;
	delete project.ui_state;
	const ids = records(project.root).map((node) => node.uuid);
	assert.equal(new Set(ids).size, ids.length, 'Expanded authored UUIDs must be unique');
	return { project, metadata };
}

/** Fill only the independently reported unrelated-tree dimension after backend calibration. */
export function pad(project, missing) {
	assert.ok(Number.isSafeInteger(missing) && missing >= 0);
	const folder = (index) => ({
		...clone({ ...project.root, children: [] }),
		meta: { label: `Unrelated ${index}`, decl_id: `unrelated_${index}` },
		children: []
	});
	if (missing === 1) project.root.children.push(folder(0));
	if (missing > 1) {
		const container = folder('records');
		for (let index = 0; index < missing - 1; index++) container.children.push(folder(index));
		project.root.children.push(container);
	}
}
