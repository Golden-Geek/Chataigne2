<script lang="ts">
	import { NodeInspector, type UiNodeDto } from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import type {
		MappingOutputBindingsDto,
		MappingOutputSourceDto,
		MappingOutputTargetDto,
		MappingValueShapeDto
	} from '../../state_machine/generated';
	import MappingValueSourceEditor from './MappingValueSourceEditor.svelte';

	let {
		node,
		shape,
		target
	}: {
		node: UiNodeDto;
		shape: MappingValueShapeDto | null;
		target: MappingOutputTargetDto | null;
	} = $props();
	let session = $derived(appState.session);
	let liveNode = $derived(session?.graph.state.nodesById.get(node.node_id) ?? node);
	let document = $derived.by((): MappingOutputBindingsDto | null => {
		if (liveNode.data.kind !== 'parameter' || liveNode.data.param.value.kind !== 'str') return null;
		try {
			const parsed: unknown = JSON.parse(liveNode.data.param.value.value);
			if (
				!parsed ||
				typeof parsed !== 'object' ||
				!('value' in parsed) ||
				!parsed.value ||
				typeof parsed.value !== 'object' ||
				!('kind' in parsed.value) ||
				!('arguments' in parsed) ||
				!Array.isArray(parsed.arguments) ||
				!('send_policy' in parsed) ||
				!['every_delivery', 'on_change'].includes(String(parsed.send_policy))
			)
				return null;
			return parsed as MappingOutputBindingsDto;
		} catch {
			return null;
		}
	});
	let pending = $state(false);
	let error = $state('');
	let draftParameterId = $state('');
	let draftSource = $state<MappingOutputSourceDto | null>(null);
	let availableArguments = $derived(
		target?.arguments.filter(
			(candidate) =>
				!document?.arguments.some((binding) => binding.parameter.stable_id === candidate.id)
		) ?? []
	);

	const commit = async (next: MappingOutputBindingsDto): Promise<void> => {
		if (!session || pending) return;
		pending = true;
		error = '';
		try {
			await session.sendIntent({
				kind: 'setParam',
				node: liveNode.node_id,
				value: { kind: 'str', value: JSON.stringify(next) },
				behaviour: 'Coalesce'
			});
		} catch (reason) {
			error = reason instanceof Error ? reason.message : String(reason);
		} finally {
			pending = false;
		}
	};

	const addBinding = (): void => {
		if (!document || !draftSource) return;
		const candidate = availableArguments.find((argument) => argument.id === draftParameterId);
		if (!candidate) return;
		void commit({
			...document,
			arguments: [
				...document.arguments,
				{
					parameter: { stable_id: candidate.id, value_type: candidate.value_type },
					source: draftSource
				}
			]
		});
		draftParameterId = '';
		draftSource = null;
	};
</script>

{#if document}
	<div class="mapping-output-bindings" aria-label="Command argument bindings">
		<label class="binding-field">
			<span>Result value</span>
			<MappingValueSourceEditor
				source={document.value}
				{shape}
				disabled={pending}
				onCommit={(value) => void commit({ ...document, value })} />
		</label>
		{#if shape?.elements.length !== 1 && document.value.kind === 'whole' && document.arguments.length > 0}
			<small>Arguments are sent with a Unit result payload.</small>
		{/if}
		<label class="binding-field">
			<span>Send</span>
			<select
				value={document.send_policy}
				disabled={pending}
				aria-label="Output send policy"
				onchange={(event) =>
					void commit({
						...document,
						send_policy: event.currentTarget.value as MappingOutputBindingsDto['send_policy']
					})}>
				<option value="every_delivery">Every delivery</option>
				<option value="on_change">When value changes</option>
			</select>
		</label>
		{#each document.arguments as binding (binding.parameter.stable_id)}
			{@const candidate = target?.arguments.find(
				(entry) => entry.id === binding.parameter.stable_id
			)}
			<div class="binding-argument">
				<div class="binding-argument-heading">
					<strong>{candidate?.label ?? binding.parameter.stable_id}</strong>
					<button
						type="button"
						disabled={pending}
						aria-label={`Remove binding for ${candidate?.label ?? binding.parameter.stable_id}`}
						onclick={() =>
							void commit({
								...document,
								arguments: document.arguments.filter(
									(entry) => entry.parameter.stable_id !== binding.parameter.stable_id
								)
							})}>Remove</button>
				</div>
				<MappingValueSourceEditor
					source={binding.source}
					{shape}
					disabled={pending}
					onCommit={(source) =>
						void commit({
							...document,
							arguments: document.arguments.map((entry) =>
								entry.parameter.stable_id === binding.parameter.stable_id
									? { ...entry, source }
									: entry
							)
						})} />
			</div>
		{/each}
		{#if availableArguments.length > 0}
			<div class="binding-add">
				<label class="binding-field">
					<span>Command argument</span>
					<select bind:value={draftParameterId} disabled={pending} aria-label="Command argument">
						<option value="">Choose argument</option>
						{#each availableArguments as argument (argument.id)}
							<option value={argument.id}>{argument.label} · {argument.value_type}</option>
						{/each}
					</select>
				</label>
				<MappingValueSourceEditor
					source={draftSource}
					{shape}
					disabled={pending}
					onCommit={(source) => (draftSource = source)} />
				<button
					type="button"
					onclick={addBinding}
					disabled={pending || !draftParameterId || !draftSource}>Bind argument</button>
			</div>
		{:else if !target?.target_label}
			<p>Choose a command to see its argument parameters.</p>
		{/if}
		{#if target?.truncated}<p>Only the first 256 command arguments are available here.</p>{/if}
		{#if pending}<small role="status">Saving binding…</small>{/if}
		{#if error}<small role="alert">{error}</small>{/if}
	</div>
{:else}
	<div class="mapping-output-bindings" role="alert">
		<p>The binding document is invalid. Edit its JSON parameter to repair it.</p>
		<NodeInspector nodes={[liveNode]} level={0} />
	</div>
{/if}

<style>
	.mapping-output-bindings {
		display: flex;
		flex-direction: column;
		gap: 0.55rem;
		min-inline-size: 0;
	}
	.binding-field {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
		min-inline-size: 0;
	}
	.binding-field > span {
		font-size: 0.72rem;
		color: color-mix(in srgb, var(--gc-color-text) 70%, transparent);
	}
	.binding-field select,
	button {
		min-block-size: 1.8rem;
		padding: 0.2rem 0.35rem;
		border: 0.06rem solid var(--gc-color-border);
		border-radius: 0.3rem;
		background: var(--gc-color-background);
		color: var(--gc-color-text);
		font: inherit;
	}
	.binding-argument,
	.binding-add {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
		padding: 0.45rem;
		border: 0.06rem solid var(--gc-color-border);
		border-radius: 0.35rem;
	}
	.binding-argument-heading {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.3rem;
	}
	button {
		cursor: pointer;
		align-self: start;
	}
	p,
	small {
		margin: 0;
		font-size: 0.72rem;
	}
	small[role='alert'] {
		color: var(--gc-color-danger, #e55);
	}
</style>
