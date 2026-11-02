<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { UiNodeDto, NodeInspectorChildFilter } from 'golden_ui';
	let {
		defaultHeader,
		defaultContent,
		defaultChildren
	}: {
		node: UiNodeDto;
		defaultHeader?: Snippet;
		defaultContent?: Snippet<[Snippet?]>;
		defaultChildren?: Snippet<[String?, NodeInspectorChildFilter?]>;
	} = $props();
	const parametersOnly = (child: UiNodeDto) => child.decl_id === 'parameters';
</script>

{@render defaultHeader?.()}
{#snippet content()}
	{@render defaultChildren?.('', parametersOnly)}
{/snippet}
{@render defaultContent?.(content)}
