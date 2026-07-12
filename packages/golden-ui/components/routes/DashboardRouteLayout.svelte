<script lang="ts">
	import type { Snippet } from 'svelte';

	import WorkbenchSessionRoot from '../app/WorkbenchSessionRoot.svelte';

	let { children } = $props<{
		children?: Snippet;
	}>();
</script>

<WorkbenchSessionRoot enableGlobalShortcuts={false}>
	{#if children}
		{@render children()}
	{/if}
</WorkbenchSessionRoot>
