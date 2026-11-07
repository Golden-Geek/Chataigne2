<script lang="ts">
	import { appState, resolveParameterEditor, type UiNodeDto } from 'golden_ui';
	let { media, disabled }: { media: string; disabled: boolean } = $props();
	const session = $derived(appState.session);
	const graph = $derived(session?.graph.state);
	const owner = $derived(graph?.nodesById.getByUuid(media));
	function child(parent: UiNodeDto | undefined, declaration: string) {
		if (!parent || !graph) return undefined;
		for (const id of graph.structuralChildrenById.get(parent.node_id) ?? []) {
			const node = graph.nodesById.get(id);
			if (node?.decl_id === declaration) return node;
		}
		return undefined;
	}
	const parameters = $derived(child(owner, 'parameters'));
	const trigger = $derived(child(parameters, 'parameters/stop_all'));
	const Editor = $derived(resolveParameterEditor(trigger, graph?.nodesById)?.component);
	const readOnly = $derived(
		disabled ||
			!session ||
			session.status !== 'connected' ||
			session.loading.visible ||
			session.historyBusy ||
			!owner?.meta.enabled ||
			trigger?.data.kind !== 'parameter' ||
			trigger.data.param.read_only ||
			!['manual', 'templateText'].includes(trigger.data.param.control.mode)
	);
</script>

<div
	class="stop-all"
	title="Stop All"
	data-matrix-stop-all
	data-parameter-uuid={trigger?.uuid}
	data-parameter-id={trigger?.node_id}>
	{#if trigger && Editor}
		<Editor node={trigger} layoutMode="widget" insideLabel="Stop All" {readOnly} />
	{:else}
		<button type="button" disabled title="Matrix controls unavailable">Stop All</button>
	{/if}
</div>

<style>
	.stop-all {
		inline-size: 100%;
		block-size: 1.65rem;
		min-inline-size: 0;
	}
</style>
