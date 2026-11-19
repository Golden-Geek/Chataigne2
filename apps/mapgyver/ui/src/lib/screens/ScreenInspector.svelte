<script lang="ts">
	import type { Snippet } from 'svelte';
	import { appState, type UiNodeDto, type NodeInspectorChildFilter } from 'golden_ui';
	let {
		node,
		defaultHeader,
		defaultContent,
		defaultChildren
	}: {
		node: UiNodeDto;
		defaultHeader?: Snippet<[Snippet?]>;
		defaultContent?: Snippet<[Snippet?]>;
		defaultChildren?: Snippet<[String?, NodeInspectorChildFilter?]>;
	} = $props();
	const parametersOnly = (child: UiNodeDto) => child.decl_id === 'parameters';
	const ndi = $derived.by(() => {
		if (node.node_type !== 'mapgyver.screen') return false;
		const session = appState.session;
		const folder = node.children
			.map((id) => session?.getNodeData(id))
			.find((child) => child?.decl_id === 'parameters');
		const target = folder?.children
			.map((id) => session?.getNodeData(id))
			.find((child) => child?.decl_id === 'parameters/target_kind');
		const value = target?.data.kind === 'parameter' ? target.data.param.value : null;
		return value?.kind === 'enum' && value.value === 'NDI';
	});
</script>

{#snippet ndiLink()}
	{#if ndi}
		<a href="https://ndi.video/" target="_blank" rel="noopener noreferrer" aria-label="NDI website">
			NDI
		</a>
	{/if}
{/snippet}
{@render defaultHeader?.(ndi ? ndiLink : undefined)}
{#snippet content()}
	{@render defaultChildren?.('', parametersOnly)}
{/snippet}
{@render defaultContent?.(content)}
