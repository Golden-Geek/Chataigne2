<script lang="ts">
	import type { MappingOutputSourceDto, MappingValueShapeDto } from '../../state_machine/generated';
	import { mappingSourceChoices, sourceChoiceKey } from '../mappingInspectorModel';
	import MappingConstantEditor from './MappingConstantEditor.svelte';

	let {
		source,
		shape,
		disabled = false,
		onCommit
	}: {
		source: MappingOutputSourceDto | null;
		shape: MappingValueShapeDto | null;
		disabled?: boolean;
		onCommit: (source: MappingOutputSourceDto) => void;
	} = $props();

	let choices = $derived(shape ? mappingSourceChoices(shape) : []);
	let selected = $derived(source ? sourceChoiceKey(source) : '');
	let constantDraft = $state(false);
	let known = $derived(
		selected === 'constant' || choices.some((choice) => choice.key === selected)
	);

	const selectSource = (key: string): void => {
		if (key === 'constant') {
			constantDraft = true;
			return;
		}
		constantDraft = false;
		const choice = choices.find((candidate) => candidate.key === key);
		if (choice) onCommit(choice.source);
	};
</script>

<div class="mapping-value-source-editor">
	<select
		value={constantDraft ? 'constant' : selected}
		{disabled}
		aria-label="Mapping value source"
		onchange={(event) => selectSource(event.currentTarget.value)}>
		<option value="">Choose value</option>
		{#if source && !known}<option value={selected}>Unavailable binding</option>{/if}
		{#each choices as choice (choice.key)}
			<option value={choice.key}>{choice.label}</option>
		{/each}
		<option value="constant">Constant</option>
	</select>
	{#if source?.kind === 'constant' || constantDraft}
		<MappingConstantEditor
			value={source?.kind === 'constant' ? source.value : null}
			{disabled}
			onCommit={(value) => {
				constantDraft = false;
				onCommit({ kind: 'constant', value });
			}} />
	{/if}
</div>

<style>
	.mapping-value-source-editor {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
		min-inline-size: 0;
	}
	select {
		inline-size: 100%;
		min-block-size: 1.8rem;
		padding: 0.2rem 0.35rem;
		border: 0.06rem solid var(--gc-color-border);
		border-radius: 0.3rem;
		background: var(--gc-color-background);
		color: var(--gc-color-text);
		font: inherit;
	}
</style>
