<script lang="ts">
	import type { GraphState } from '../store/graph';
	import type { ParamValue, UiEditIntent, UiNodeDto } from '../types';

	let {
		state,
		onIntent
	}: {
		state: GraphState;
		onIntent: (intent: UiEditIntent) => void;
	} = $props();

	const selectedNode = $derived(
		state.selectedNodeId !== null ? state.nodesById.get(state.selectedNodeId) ?? null : null
	);
	const selectedParam = $derived(
		selectedNode && selectedNode.data.kind === 'parameter' ? selectedNode.data.param : null
	);
	const selectedEnumVariantId = $derived(
		selectedParam
			? selectedParam.constraints.enum_options.find(
					(option) => JSON.stringify(option.value) === JSON.stringify(selectedParam.value)
				)?.variant_id
			: undefined
	);

	const dispatchSetParam = (node: UiNodeDto, value: ParamValue): void => {
		if (node.data.kind !== 'parameter') {
			return;
		}
		onIntent({
			kind: 'setParam',
			node: node.node_id,
			value,
			behaviour: node.data.param.event_behaviour
		});
	};

	const dispatchEnableToggle = (node: UiNodeDto, enabled: boolean): void => {
		onIntent({
			kind: 'patchMeta',
			node: node.node_id,
			patch: { enabled }
		});
	};
</script>

<section class="inspector-panel">
	<header class="inspector-header">
		<h2>Inspector</h2>
	</header>
	{#if selectedNode}
		<div class="meta">
			<p class="label">{selectedNode.meta.label}</p>
			<p class="subtitle">{selectedNode.node_type}</p>
		</div>
		<div class="field">
			<label for="node-enabled">Enabled</label>
			<input
				id="node-enabled"
				type="checkbox"
				checked={selectedNode.meta.enabled}
				disabled={!selectedNode.meta.can_be_disabled}
				onchange={(event) =>
					dispatchEnableToggle(selectedNode, (event.currentTarget as HTMLInputElement).checked)}
			/>
		</div>

		{#if selectedParam}
			<div class="field">
				<p class="field-label">Value</p>
				{#if selectedParam.value.kind === 'bool'}
					<input
						type="checkbox"
						checked={selectedParam.value.value}
						disabled={selectedParam.read_only}
						onchange={(event) =>
							dispatchSetParam(selectedNode, {
								kind: 'bool',
								value: (event.currentTarget as HTMLInputElement).checked
							})}
					/>
				{:else if selectedParam.value.kind === 'int'}
					<input
						type="number"
						value={selectedParam.value.value}
						min={selectedParam.constraints.min}
						max={selectedParam.constraints.max}
						step={selectedParam.constraints.step ?? 1}
						disabled={selectedParam.read_only}
						onchange={(event) =>
							dispatchSetParam(selectedNode, {
								kind: 'int',
								value: Number((event.currentTarget as HTMLInputElement).value)
							})}
					/>
				{:else if selectedParam.value.kind === 'float'}
					<input
						type="number"
						value={selectedParam.value.value}
						min={selectedParam.constraints.min}
						max={selectedParam.constraints.max}
						step={selectedParam.constraints.step ?? 0.01}
						disabled={selectedParam.read_only}
						onchange={(event) =>
							dispatchSetParam(selectedNode, {
								kind: 'float',
								value: Number((event.currentTarget as HTMLInputElement).value)
							})}
					/>
				{:else if selectedParam.value.kind === 'str'}
					<input
						type="text"
						value={selectedParam.value.value}
						disabled={selectedParam.read_only}
						onchange={(event) =>
							dispatchSetParam(selectedNode, {
								kind: 'str',
								value: (event.currentTarget as HTMLInputElement).value
							})}
					/>
				{:else}
					<pre>{JSON.stringify(selectedParam.value)}</pre>
				{/if}

				{#if selectedParam.constraints.enum_options.length > 0}
					<select
						value={selectedEnumVariantId}
						disabled={selectedParam.read_only}
						onchange={(event) => {
							const variantId = (event.currentTarget as HTMLSelectElement).value;
							const variant = selectedParam.constraints.enum_options.find(
								(option) => option.variant_id === variantId
							);
							if (variant) {
								dispatchSetParam(selectedNode, variant.value);
							}
						}}
					>
						{#each selectedParam.constraints.enum_options as option (option.variant_id)}
							<option value={option.variant_id}>
								{option.label}
							</option>
						{/each}
					</select>
				{/if}
			</div>
			<p class="hint">
				event: {selectedParam.event_behaviour} | constraints: {selectedParam.constraints.policy}
			</p>
		{/if}
	{:else}
		<p class="empty">Select a node to inspect details.</p>
	{/if}
</section>

<style>
	.inspector-panel {
		background: var(--panel-bg);
		border: 1px solid var(--panel-border);
		border-radius: 14px;
		padding: 0.85rem;
	}

	.inspector-header {
		padding-bottom: 0.65rem;
	}

	.inspector-header h2 {
		margin: 0;
		font-size: 0.92rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
	}

	.meta {
		margin-bottom: 0.75rem;
	}

	.label {
		margin: 0;
		font-size: 1.05rem;
		font-weight: 700;
	}

	.subtitle {
		margin: 0.15rem 0 0;
		font-size: 0.78rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		opacity: 0.65;
	}

	.field {
		display: grid;
		grid-template-columns: 1fr;
		gap: 0.35rem;
		margin-bottom: 0.7rem;
	}

	.field label {
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		opacity: 0.75;
	}

	.field-label {
		margin: 0;
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		opacity: 0.75;
	}

	input[type='text'],
	input[type='number'],
	select {
		width: 100%;
		background: color-mix(in srgb, var(--panel-bg) 75%, white 25%);
		color: inherit;
		border: 1px solid var(--panel-border);
		border-radius: 8px;
		padding: 0.4rem 0.45rem;
	}

	.hint {
		margin: 0;
		font-size: 0.75rem;
		opacity: 0.7;
	}

	.empty {
		margin: 0;
		opacity: 0.75;
	}

	pre {
		margin: 0;
		white-space: pre-wrap;
		word-break: break-word;
		font-size: 0.75rem;
	}
</style>
