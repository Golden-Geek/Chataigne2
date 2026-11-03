<script lang="ts">
	import { useMatrixAuthoring } from './context.svelte';
	import RuntimeFeedback from './RuntimeFeedback.svelte';
	const owner = useMatrixAuthoring();
	const snapshot = $derived(owner.matrixWindow);
</script>

{#if snapshot}<RuntimeFeedback {snapshot} observation={owner.observation} />{/if}
