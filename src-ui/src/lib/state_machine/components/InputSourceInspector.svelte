<script lang="ts">
	import type { NodeInspectorComponentProps } from 'golden_ui';
	import ReferencedParameterHeaderControl from './ReferencedParameterHeaderControl.svelte';

	let { node, defaultHeader, defaultContent, defaultChildren }: NodeInspectorComponentProps =
		$props();
</script>

{#snippet inputSourceHeaderExtra()}
	<span
		class="input-source-header-extra"
		role="presentation"
		onclick={(event) => event.stopPropagation()}
		onkeydown={(event) => event.stopPropagation()}>
		<ReferencedParameterHeaderControl owner={node} />
	</span>
{/snippet}

{@render defaultHeader?.(inputSourceHeaderExtra)}

{#snippet inputSourceContent()}
	{@render defaultChildren?.()}
{/snippet}

{@render defaultContent?.(inputSourceContent, 'input-source-inspector')}

<style>
	.input-source-header-extra {
		display: inline-flex;
		align-items: center;
		min-inline-size: 0;
		margin-left: 0.2rem;
	}

	:global(.input-source-inspector) {
		min-inline-size: 0;
	}
</style>
