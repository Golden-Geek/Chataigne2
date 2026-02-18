<script lang="ts">
	import { getWorkbenchContext } from '../store/workbench-context';
	import type { ParamEventBehaviour, ParamValue, UiNodeDto } from '../types';

	let {
		node,
		title = null
	}: {
		node: UiNodeDto;
		title?: string | null;
	} = $props();

	const session = getWorkbenchContext();
	const param = $derived(node.data.kind === 'parameter' ? node.data.param : null);
	const selectedEnumVariantId = $derived.by(() => {
		if (!param) {
			return undefined;
		}

		if (param.value.kind === 'enum') {
			return param.value.value;
		}

		const exactValueMatch = param.constraints.enum_options.find(
			(option) => JSON.stringify(option.value) === JSON.stringify(param.value)
		);
		if (exactValueMatch) {
			return exactValueMatch.variant_id;
		}

		if (param.value.kind === 'str') {
			const variantId = param.value.value;
			return param.constraints.enum_options.find(
				(option) => option.variant_id === variantId
			)?.variant_id;
		}

		return undefined;
	});

	let activeContinuousEditId: string | null = null;

	const createClientEditId = (currentNode: UiNodeDto): string => {
		if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
			return `drag-${currentNode.node_id}-${crypto.randomUUID()}`;
		}
		return `drag-${currentNode.node_id}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
	};

	const beginContinuousEdit = (currentNode: UiNodeDto): void => {
		if (activeContinuousEditId !== null) {
			return;
		}
		const clientEditId = createClientEditId(currentNode);
		activeContinuousEditId = clientEditId;
		void session.sendIntent({
			kind: 'beginEdit',
			client_edit_id: clientEditId,
			label: `Adjust ${currentNode.meta.label}`
		});
	};

	const endContinuousEdit = (): void => {
		if (activeContinuousEditId === null) {
			return;
		}

		void session.sendIntent({
			kind: 'endEdit',
			client_edit_id: activeContinuousEditId
		});
		activeContinuousEditId = null;
	};

	const dispatchSetParam = (
		currentNode: UiNodeDto,
		value: ParamValue,
		behaviourOverride?: ParamEventBehaviour
	): void => {
		if (currentNode.data.kind !== 'parameter') {
			return;
		}
		void session.sendIntent({
			kind: 'setParam',
			node: currentNode.node_id,
			value,
			behaviour: behaviourOverride ?? currentNode.data.param.event_behaviour
		});
	};
</script>

{#if param}
	<article class="param-card">
		<header class="param-header">
			<p class="param-label">{title ?? node.meta.label}</p>
			<p class="param-subtitle">{node.decl_id}</p>
		</header>

		<div class="field">
			<p class="field-label">Value</p>
			{#if param.value.kind === 'bool'}
				<input
					type="checkbox"
					checked={param.value.value}
					disabled={param.read_only}
					onchange={(event) =>
						dispatchSetParam(node, {
							kind: 'bool',
							value: (event.currentTarget as HTMLInputElement).checked
						})}
				/>
			{:else if param.value.kind === 'int'}
				<input
					type="number"
					value={param.value.value}
					min={param.constraints.min}
					max={param.constraints.max}
					step={param.constraints.step ?? 1}
					disabled={param.read_only}
					onchange={(event) =>
						dispatchSetParam(node, {
							kind: 'int',
							value: Number((event.currentTarget as HTMLInputElement).value)
						})}
				/>
			{:else if param.value.kind === 'float'}
				<input
					type={param.constraints.min !== undefined && param.constraints.max !== undefined ? 'range' : 'number'}
					value={param.value.value}
					min={param.constraints.min}
					max={param.constraints.max}
					step={param.constraints.step ?? 0.01}
					disabled={param.read_only}
					onpointerdown={() => {
						if (param.constraints.min !== undefined && param.constraints.max !== undefined) {
							beginContinuousEdit(node);
						}
					}}
					oninput={(event) => {
						if (param.constraints.min !== undefined && param.constraints.max !== undefined) {
							dispatchSetParam(
								node,
								{
									kind: 'float',
									value: Number((event.currentTarget as HTMLInputElement).value)
								},
								'Coalesce'
							);
						}
					}}
					onchange={(event) => {
						if (param.constraints.min !== undefined && param.constraints.max !== undefined) {
							endContinuousEdit();
							return;
						}

						dispatchSetParam(
							node,
							{
								kind: 'float',
								value: Number((event.currentTarget as HTMLInputElement).value)
							},
							'Append'
						);
					}}
					onpointerup={() => endContinuousEdit()}
					onpointercancel={() => endContinuousEdit()}
					onblur={() => endContinuousEdit()}
				/>
			{:else if param.value.kind === 'enum'}
				<input
					type="text"
					value={param.value.value}
					disabled={param.read_only}
					onchange={(event) =>
						dispatchSetParam(node, {
							kind: 'enum',
							value: (event.currentTarget as HTMLInputElement).value
						})}
				/>
			{:else if param.value.kind === 'str'}
				<input
					type="text"
					value={param.value.value}
					disabled={param.read_only}
					onchange={(event) =>
						dispatchSetParam(node, {
							kind: 'str',
							value: (event.currentTarget as HTMLInputElement).value
						})}
				/>
			{:else}
				<pre>{JSON.stringify(param.value)}</pre>
			{/if}

			{#if param.constraints.enum_options.length > 0}
				<select
					value={selectedEnumVariantId}
					disabled={param.read_only}
					onchange={(event) => {
						const variantId = (event.currentTarget as HTMLSelectElement).value;
						const variant = param.constraints.enum_options.find(
							(option) => option.variant_id === variantId
						);
						if (variant) {
							dispatchSetParam(node, { kind: 'enum', value: variant.variant_id });
						}
					}}
				>
					{#each param.constraints.enum_options as option (option.variant_id)}
						<option value={option.variant_id}>
							{option.label}
						</option>
					{/each}
				</select>
			{/if}
		</div>

		<p class="hint">
			event: {param.event_behaviour} | constraints: {param.constraints.policy}
		</p>
	</article>
{/if}

<style>
	.param-card {
		background: color-mix(in srgb, var(--panel-bg) 86%, white 14%);
		border: 0.0625rem solid var(--panel-border);
		border-radius: 0.625rem;
		padding: 0.65rem;
		display: grid;
		gap: 0.55rem;
	}

	.param-header {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 0.75rem;
	}

	.param-label {
		margin: 0;
		font-weight: 600;
	}

	.param-subtitle {
		margin: 0;
		font-size: 0.68rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		opacity: 0.65;
	}

	.field {
		display: grid;
		grid-template-columns: 1fr;
		gap: 0.35rem;
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
		border: 0.0625rem solid var(--panel-border);
		border-radius: 0.5rem;
		padding: 0.4rem 0.45rem;
	}

	.hint {
		margin: 0;
		font-size: 0.75rem;
		opacity: 0.7;
	}

	pre {
		margin: 0;
		white-space: pre-wrap;
		word-break: break-word;
		font-size: 0.75rem;
	}
</style>
