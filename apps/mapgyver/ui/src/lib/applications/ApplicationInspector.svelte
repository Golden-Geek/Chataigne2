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
		defaultHeader?: Snippet;
		defaultContent?: Snippet<[Snippet?]>;
		defaultChildren?: Snippet<[String?, NodeInspectorChildFilter?]>;
	} = $props();
	const session = $derived(appState.session);
	const configured = $derived.by(() => {
		const folder = node.children
			.map((id) => session?.getNodeData(id))
			.find((child) => child?.decl_id === 'parameters');
		const file = folder?.children
			.map((id) => session?.getNodeData(id))
			.find((child) => child?.decl_id === 'parameters/app_file');
		return (
			file?.data.kind === 'parameter' &&
			file.data.param.value.kind === 'file' &&
			!!file.data.param.value.value
		);
	});
</script>

{@render defaultHeader?.()}
{#snippet content()}
	{@render defaultChildren?.('', (child) => child.decl_id === 'parameters' || configured)}
{/snippet}
{@render defaultContent?.(content)}
