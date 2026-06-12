<script lang="ts">
	import type { UiNodeDto } from 'golden_ui';
	import ParameterInspector from 'golden_ui/components/panels/inspector/ParameterInspector.svelte';

	let { parameters }: { parameters: UiNodeDto[] } = $props();

	const orderFor = (index: number): 'first' | 'last' | 'solo' | '' => {
		if (parameters.length === 1) return 'solo';
		if (index === 0) return 'first';
		if (index === parameters.length - 1) return 'last';
		return '';
	};
</script>

{#if parameters.length > 0}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="config"
		onpointerdown={(event) => event.stopPropagation()}
		onkeydown={(event) => event.stopPropagation()}>
		{#each parameters as parameter, index (parameter.node_id)}
			<ParameterInspector node={parameter} level={0} order={orderFor(index)} />
		{/each}
	</div>
{/if}

<style>
	.config {
		display: grid;
		border-block-start: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 70%, transparent);
	}
</style>
