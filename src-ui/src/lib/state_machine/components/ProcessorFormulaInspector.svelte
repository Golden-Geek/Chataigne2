<script lang="ts">
	import { showPanel, type NodeInspectorComponentProps } from 'golden_ui';

	let { node, defaultHeader, defaultChildren, collapsed }: NodeInspectorComponentProps = $props();

	let formulaKind = $derived(node.node_type === 'state_processor_action' ? 'Action' : 'Mapping');
	let formulaDescription = $derived(
		formulaKind === 'Action'
			? 'Evaluate a condition and invoke edge-specific consequences.'
			: 'Transform an input range, smooth it, and send it to an output target.'
	);

	const openAlchemistEditor = (): void => {
		showPanel({
			panelId: 'alchemist-editor',
			panelType: 'alchemistEditor',
			title: `Alchemist: ${node.meta.label}`,
			params: {
				processorNodeId: node.node_id
			},
			position: {
				referencePanelId: 'state-machine',
				direction: 'within'
			}
		});
	};
</script>

{#snippet formulaHeaderExtra()}
	<span class="formula-kind">{formulaKind}</span>
{/snippet}

{@render defaultHeader?.(formulaHeaderExtra)}

{#if collapsed !== true}
	<div class="processor-formula-inspector">
		<div class="formula-summary">
			<div>
				<strong>{formulaKind} Formula</strong>
				<p>{formulaDescription}</p>
			</div>
			<button type="button" onclick={openAlchemistEditor}>Open Alchemist Editor</button>
		</div>
		{@render defaultChildren?.()}
	</div>
{/if}

<style>
	.formula-kind {
		margin-inline-start: 0.3rem;
		padding: 0.12rem 0.34rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-accent) 45%, transparent);
		border-radius: 999rem;
		color: var(--gc-color-accent);
		font-size: 0.62rem;
		font-weight: 650;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}

	.processor-formula-inspector {
		min-inline-size: 0;
	}

	.formula-summary {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.7rem;
		margin: 0.35rem 0.45rem 0.5rem;
		padding: 0.65rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 72%, transparent);
		border-radius: 0.45rem;
		background: color-mix(in srgb, var(--gc-color-background-soft) 76%, transparent);
	}

	.formula-summary strong {
		font-size: 0.75rem;
	}

	.formula-summary p {
		max-inline-size: 24rem;
		margin: 0.18rem 0 0;
		color: color-mix(in srgb, var(--gc-color-text) 68%, transparent);
		font-size: 0.66rem;
		line-height: 1.35;
	}

	button {
		flex: 0 0 auto;
		min-block-size: 1.8rem;
		padding: 0.35rem 0.65rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-accent) 52%, transparent);
		border-radius: 0.35rem;
		background: color-mix(in srgb, var(--gc-color-accent) 18%, transparent);
		color: var(--gc-color-text);
		font: inherit;
		font-size: 0.68rem;
		cursor: pointer;
	}

	button:hover {
		background: color-mix(in srgb, var(--gc-color-accent) 28%, transparent);
	}
</style>
