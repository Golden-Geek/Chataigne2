<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { UiNodeDto } from 'golden_ui';
	import ProcessorFormulaInspector from '../../components/ProcessorFormulaInspector.svelte';

	let { node }: { node: UiNodeDto } = $props();
</script>

{#snippet header(extra: Snippet | undefined)}
	{@render extra?.()}
{/snippet}

{#snippet content(inner: Snippet | undefined)}
	{@render inner?.()}
{/snippet}

{#snippet children()}
	<div aria-label="Ordinary processor children">Ordinary manager and parameter nodes</div>
{/snippet}

<ProcessorFormulaInspector
	{node}
	level={0}
	order="solo"
	defaultHeader={header}
	defaultContent={content}
	defaultChildren={children} />
