<script lang="ts">
	import { NodeInspector, type UiNodeDto } from 'golden_ui';

	let {
		title,
		description = null,
		node,
		open = true
	} = $props<{
		title: string;
		description?: string | null;
		node: UiNodeDto | null;
		open?: boolean;
	}>();
</script>

<details class="node-section" {open}>
	<summary>
		<span>{title}</span>
		{#if description}<small>{description}</small>{/if}
	</summary>
	<div class="node-section-content">
		{#if node}
			<NodeInspector nodes={[node]} level={1} order="solo" />
		{:else}
			<p class="empty">This section is not available in the current module snapshot.</p>
		{/if}
	</div>
</details>

<style>
	.node-section {
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.45rem;
		background: color-mix(in srgb, var(--gc-color-bg-light) 82%, transparent);
		overflow: clip;
	}

	summary {
		display: flex;
		align-items: baseline;
		gap: 0.65rem;
		padding: 0.7rem 0.8rem;
		cursor: pointer;
		font-weight: 650;
	}

	summary:focus-visible {
		outline: 0.15rem solid var(--gc-color-accent);
		outline-offset: -0.15rem;
	}

	summary small {
		color: var(--gc-color-text-muted);
		font-size: 0.72rem;
		font-weight: 400;
	}

	.node-section-content {
		padding: 0 0.45rem 0.55rem;
	}

	.empty {
		margin: 0;
		padding: 0.75rem;
		color: var(--gc-color-text-muted);
		font-size: 0.8rem;
	}
</style>
