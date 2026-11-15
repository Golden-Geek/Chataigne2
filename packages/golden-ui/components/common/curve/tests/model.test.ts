import { test } from 'vitest';
import assert from 'node:assert/strict';
import { fixtureNode } from '../../../../tests/browser/fixture/session';
import {
	parameterValueReadOnly,
	canEditParameterValue
} from '../../../../store/parameter-value-access';
import type { ParamValue } from '../../../../types';
import { readCurveKeys } from '../model';
import { curveGeometryParameters, curveParametersWritable } from '../admission';
import { to_param_ref } from '../values';

function parameter(id: number, declaration: string, value: ParamValue) {
	const node = fixtureNode(id, declaration, [], 'parameter');
	node.decl_id = declaration;
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
	return node;
}
function scene() {
	const curve = fixtureNode(1, 'Curve', [2], 'animation_curve');
	const position = parameter(3, 'position', { kind: 'float', value: 0 });
	const value = parameter(4, 'value', { kind: 'float', value: 1 });
	const easing = fixtureNode(5, 'Easing', [6, 7, 8, 9, 10], 'animation_curve_easing');
	easing.decl_id = 'easing';
	const nodes = new Map(
		[
			curve,
			fixtureNode(2, 'Key', [3, 4, 5], 'animation_curve_key'),
			position,
			value,
			easing,
			parameter(6, 'kind', { kind: 'enum', value: 'bezier' }),
			...['out_position', 'out_value', 'in_position', 'in_value'].map((decl, index) =>
				parameter(7 + index, decl, { kind: 'float', value: 0 })
			)
		].map((node) => [node.node_id, node])
	);
	const read = () => readCurveKeys(nodes, curve, new Map(), new Map());
	return { curve, position, value, nodes, read };
}

test('typed Curve projection preserves document values, UUIDs and read-only selected keys', () => {
	const f = scene();
	if (f.position.data.kind !== 'parameter') throw Error('fixture');
	f.position.data.param.read_only = true;
	const keys = readCurveKeys(
		f.nodes,
		f.curve,
		new Map([[2, { position: 0.5, value: 2 }]]),
		new Map([[2, { out_position: 0.2, out_value: 0.4, in_position: -0.2, in_value: -0.4 }]])
	);
	assert.equal(keys.length, 1);
	assert.equal(keys[0].position, 0.5);
	assert.equal(keys[0].position_param?.uuid, f.position.uuid);
	assert.equal(keys[0].easing?.params.out_value.value.kind, 'float');
	assert.deepEqual(f.position.data.param.value, { kind: 'float', value: 0 });
	assert.deepEqual(
		f.nodes.get(8)?.data,
		parameter(8, 'out_value', { kind: 'float', value: 0 }).data
	);
	assert.deepEqual(readCurveKeys(null, f.curve, new Map(), new Map()), []);
});

test('geometry admission requires the complete current parameter group and matching UUIDs', () => {
	const f = scene(),
		key = f.read()[0];
	const refs = curveGeometryParameters([key], [key.easing?.params]);
	assert(refs);
	assert.equal(refs.length, 6);
	assert(curveParametersWritable(f.nodes, refs));
	assert.equal(curveGeometryParameters([undefined]), null);
	assert.equal(curveGeometryParameters([], [{}]), null);
	assert.equal(curveParametersWritable(f.nodes, []), false);
	f.nodes.set(4, { ...f.value, uuid: 'replacement' });
	assert.equal(curveParametersWritable(f.nodes, refs), false);
});

test('cached references cannot retain access after disabled/read-only/control changes', () => {
	for (const boundary of ['disabled', 'readonly', 'controlled'] as const) {
		const f = scene(),
			refs = curveGeometryParameters(f.read());
		assert(refs && curveParametersWritable(f.nodes, refs));
		if (f.value.data.kind !== 'parameter') throw Error('fixture');
		if (boundary === 'disabled') f.value.meta.enabled = false;
		if (boundary === 'readonly') f.value.data.param.read_only = true;
		if (boundary === 'controlled')
			f.value.data.param.control = { mode: 'expression', spec: { mode: 'expression' } };
		assert.equal(curveParametersWritable(f.nodes, refs), false, boundary);
		assert.equal(f.read().length, 1, 'refused edits never hide selectable keys');
	}
});

test('specialized value access matches the ordinary Inspector manual/template and control rules', () => {
	const node = parameter(3, 'Value', { kind: 'float', value: 1 });
	if (node.data.kind !== 'parameter') throw Error('fixture');
	for (const mode of ['manual', 'expression', 'proxy', 'binding', 'animation'] as const) {
		node.data.param.control = { mode, spec: { mode } };
		assert.equal(canEditParameterValue(node), mode === 'manual');
	}
	node.data.param.control = {
		mode: 'templateText',
		spec: { mode: 'templateText', template: '{value}' }
	};
	assert(canEditParameterValue(node));
	node.meta.enabled = false;
	assert.equal(canEditParameterValue(node), false);
	assert.equal(parameterValueReadOnly(null), true);
	assert.equal(to_param_ref(fixtureNode(50, 'Container')), undefined);
});
