<script lang="ts">
	import { appState, resolveParameterEditor, type UiNodeDto } from 'golden_ui';
	let { uuid, label }: { uuid: string; label: string } = $props();
	const session = $derived(appState.session);
	const graph = $derived(session?.graph.state);
	const element = $derived(graph?.nodesById.getByUuid(uuid));
	// Declaration-only indexes exclude the Layer's potentially large Cell collection.
	function child(parent: UiNodeDto | undefined, declaration: string) {
		if (!parent || !graph) return undefined;
		for (const id of graph.structuralChildrenById.get(parent.node_id) ?? []) {
			const node = graph.nodesById.get(id);
			if (node?.decl_id === declaration) return node;
		}
		return undefined;
	}
	const parameters = $derived(child(element, 'parameters'));
	const opacity = $derived(child(parameters, 'parameters/opacity'));
	const blend = $derived(child(parameters, 'parameters/blend'));
	const OpacityEditor = $derived(resolveParameterEditor(opacity, graph?.nodesById)?.component);
	const BlendEditor = $derived(resolveParameterEditor(blend, graph?.nodesById)?.component);
	function readOnly(node: UiNodeDto) {
		const param = node.data.kind === 'parameter' ? node.data.param : null;
		return (
			!session ||
			session.status !== 'connected' ||
			session.loading.visible ||
			session.historyBusy ||
			!param ||
			param.read_only ||
			!['manual', 'templateText'].includes(param.control.mode)
		);
	}
</script>

<div
	class="matrix-row-controls"
	data-matrix-row-controls
	role="group"
	aria-label={`Controls for ${label}`}
	draggable="false">
	{#if opacity && OpacityEditor}
		<div
			class="control"
			role="group"
			aria-label={opacity.meta.label}
			data-matrix-parameter="opacity"
			data-parameter-uuid={opacity.uuid}
			data-parameter-id={opacity.node_id}>
			<span>{opacity.meta.label}</span>
			<div class="editor">
				<OpacityEditor
					node={opacity}
					layoutMode="widget"
					readOnly={readOnly(opacity)}
					presentation={{ compact: true }} />
			</div>
		</div>
	{/if}
	{#if blend && BlendEditor}
		<div
			class="control"
			role="group"
			aria-label={blend.meta.label}
			data-matrix-parameter="blend"
			data-parameter-uuid={blend.uuid}
			data-parameter-id={blend.node_id}>
			<span>{blend.meta.label}</span>
			<div class="editor">
				<BlendEditor node={blend} layoutMode="widget" readOnly={readOnly(blend)} />
			</div>
		</div>
	{/if}
</div>

<style>
	.matrix-row-controls {
		display: grid;
		gap: 0.25rem;
		min-inline-size: 0;
	}
	.control {
		display: flex;
		align-items: center;
		gap: 0.3rem;
		min-inline-size: 0;
		font-size: 0.8em;
	}
	.control > span {
		flex: 0 0 4.5em;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.editor {
		flex: 1;
		min-inline-size: 0;
		block-size: 1.5rem;
	}
</style>
