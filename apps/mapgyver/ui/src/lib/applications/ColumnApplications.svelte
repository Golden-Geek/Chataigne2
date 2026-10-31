<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type { WorkbenchSession } from 'golden_ui';
	import type { ColumnSnapshot } from '../media/generated/ColumnSnapshot';
	import type { MatrixEdit } from '../media/generated/MatrixEdit';
	import ApplicationPicker from './ApplicationPicker.svelte';
	import { ApplicationReferences, referenceState } from './references';
	import { columnDistances, associateApplication, removeAssociation } from './column-form';
	import { invokeMediaEdit } from '../media/operations';
	import { MEDIA_NAMESPACE } from '../media/generated/constants';
	let {
		column,
		session,
		disabled,
		edit
	}: {
		column: ColumnSnapshot;
		session: WorkbenchSession | null;
		disabled: boolean;
		edit(value: MatrixEdit): unknown;
	} = $props();
	let selected = $state<string | null>(null);
	let error = $state('');
	const references = $state(referenceState());
	const reader = new ApplicationReferences(
		() => session,
		() => column.definition.applications.map((entry) => entry.application),
		references,
		invokeMediaEdit
	);
	const labels = $derived(
		new Map(references.references.map((reference) => [reference.application, reference.summary]))
	);
	$effect(() => {
		void column.definition.applications;
		const current = session;
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.currentHistoryStateId;
			void current.hasActiveEditSession;
			void current.editSessionEpoch;
			void current.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		untrack(() => reader.refresh());
	});
	onDestroy(() => reader.dispose());
</script>

<form
	class="matrix-authoring"
	onsubmit={(event) => {
		event.preventDefault();
		const data = new FormData(event.currentTarget);
		error = '';
		try {
			edit(columnDistances(column, data));
		} catch (cause) {
			error = String(cause);
		}
	}}>
	<fieldset {disabled}>
		<legend>Column applications</legend>
		<p>
			Pre/post values are column distances, clamped at the Matrix ends. Shared references keep one
			process across columns and Matrices.
		</p>
		{#each column.definition.applications as association (association.application)}
			<div class="association">
				<span
					>{labels.get(association.application)?.label ??
						(labels.has(association.application)
							? 'Missing definition'
							: 'Saved application reference')}
					{#if labels.get(association.application)?.enabled === false}
						· Disabled{/if}
					<small>{association.application}</small></span>
				<label
					>Preload columns<input
						type="number"
						min="0"
						max="65535"
						step="1"
						required
						name={`pre:${association.application}`}
						value={association.pre} /></label>
				<label
					>Postload columns<input
						type="number"
						min="0"
						max="65535"
						step="1"
						required
						name={`post:${association.application}`}
						value={association.post} /></label>
				<button
					type="button"
					onclick={() => edit(removeAssociation(column, association.application))}
					>Remove association</button>
			</div>
		{:else}<p>No shared applications associated with this column.</p>{/each}
		<button disabled={disabled || column.definition.applications.length === 0}
			>Save column application distances</button>
	</fieldset>
</form>
<form
	class="matrix-authoring"
	onsubmit={(event) => {
		event.preventDefault();
		if (!selected) return;
		const data = new FormData(event.currentTarget);
		error = '';
		try {
			edit(associateApplication(column, selected, data));
		} catch (cause) {
			error = String(cause);
		}
	}}>
	<fieldset {disabled}>
		<legend>Associate a shared application</legend>
		<ApplicationPicker
			{session}
			{disabled}
			{selected}
			exclude={column.definition.applications.map((entry) => entry.application)}
			choose={(value) => {
				selected = value;
			}} />
		<label
			>Preload columns<input
				name="pre"
				type="number"
				min="0"
				max="65535"
				step="1"
				required /></label>
		<label
			>Postload columns<input
				name="post"
				type="number"
				min="0"
				max="65535"
				step="1"
				required /></label>
		<button disabled={disabled || !selected}>Add application association</button>
	</fieldset>
</form>
{#if error || references.error}<p role="alert">{error || references.error}</p>{/if}

<style>
	.association {
		display: grid;
		gap: 0.4rem;
		border: 0.05rem solid currentColor;
		padding: 0.5rem;
	}
	span {
		overflow-wrap: anywhere;
		font-size: 0.9em;
	}
	small {
		display: block;
		opacity: 0.8;
	}
</style>
