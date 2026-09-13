<script lang="ts">
	import { NodeInspector, type UiNodeDto } from 'golden_ui';
	import type { MappingOutputTargetDto, MappingValueShapeDto } from '../../state_machine/generated';
	import { directChild } from '../mappingInspectorModel';
	import MappingOutputBindingsEditor from './MappingOutputBindingsEditor.svelte';

	let {
		node,
		kind,
		outputShape,
		outputTarget,
		nodesById
	}: {
		node: UiNodeDto;
		kind: 'input_set' | 'filter_pipeline' | 'output_set';
		outputShape: MappingValueShapeDto | null;
		outputTarget: MappingOutputTargetDto | null;
		nodesById: ReadonlyMap<number, UiNodeDto>;
	} = $props();

	let config = $derived(directChild(node, 'config', nodesById));
	let inputs = $derived(directChild(node, 'inputs', nodesById));
	let configFields = $derived(
		config?.children
			.map((id) => nodesById.get(id))
			.filter((child): child is UiNodeDto =>
				Boolean(child && child.decl_id !== 'config/bindings')
			) ?? []
	);
	let bindings = $derived(config ? directChild(config, 'config/bindings', nodesById) : null);
</script>

<div class="mapping-item-editor" aria-label={`${node.meta.label} settings`}>
	{#if configFields.length > 0}
		<NodeInspector nodes={configFields} level={0} />
	{/if}
	{#if kind === 'filter_pipeline' && inputs}
		<NodeInspector nodes={[inputs]} level={0} />
	{/if}
	{#if kind === 'output_set' && bindings}
		<MappingOutputBindingsEditor node={bindings} shape={outputShape} target={outputTarget} />
	{/if}
</div>

<style>
	.mapping-item-editor {
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
		min-inline-size: 0;
		padding: 0.5rem;
		border: 0.06rem solid var(--gc-color-border);
		border-radius: 0.35rem;
	}
</style>
