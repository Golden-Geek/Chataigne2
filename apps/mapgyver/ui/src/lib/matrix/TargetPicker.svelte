<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type { WorkbenchSession } from 'golden_ui';
	import type { MatrixTargetId } from '../media/generated/MatrixTargetId';
	import type { MatrixTargetScope } from '../media/generated/MatrixTargetScope';
	import { invokeMediaEdit } from '../media/operations';
	import { MEDIA_NAMESPACE } from '../media/generated/constants';
	import { TargetReader, targetState, targetKey as key } from './target-choices';
	let {
		session,
		media,
		scope,
		selected,
		exclude = null,
		label,
		emptyLabel,
		disabled,
		choose
	}: {
		session: WorkbenchSession | null;
		media: string;
		scope: MatrixTargetScope;
		selected: MatrixTargetId | null;
		exclude?: MatrixTargetId | null;
		label: string;
		emptyLabel: string;
		disabled: boolean;
		choose(value: MatrixTargetId | null): void;
	} = $props();
	const state = $state(targetState());
	const reader = new TargetReader(
		() => session,
		() => ({ media, scope }),
		state,
		invokeMediaEdit
	);
	$effect(() => {
		const current = session;
		void media;
		void JSON.stringify(scope);
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.currentHistoryStateId;
			void current.editSessionEpoch;
			void current.hasActiveEditSession;
			void current.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		untrack(() => {
			state.start = 0;
			state.page = null;
			reader.refresh();
		});
	});
	onDestroy(() => reader.dispose());
</script>

<div class="matrix-authoring target-picker">
	<label
		>{label}<select
			{disabled}
			value={key(selected)}
			onchange={(event) => {
				const choice = state.page?.choices.find(
					(value) => key(value.id) === event.currentTarget.value
				);
				choose(choice?.id ?? (key(selected) === event.currentTarget.value ? selected : null));
			}}>
			<option value="">{emptyLabel}</option>
			{#if selected && !state.page?.choices.some((choice) => key(choice.id) === key(selected))}
				<option value={key(selected)}>Selected target</option>
			{/if}
			{#each state.page?.choices ?? [] as choice (key(choice.id))}
				<option value={key(choice.id)} disabled={key(choice.id) === key(exclude)}
					>{choice.label}</option>
			{/each}
		</select></label>
	<div class="actions">
		<input
			aria-label={`Search ${label}`}
			placeholder="Search targets…"
			bind:value={state.search}
			onkeydown={(event) => {
				if (event.key === 'Enter') {
					event.preventDefault();
					state.start = 0;
					reader.refresh();
				}
			}}
			{disabled} />
		<button
			type="button"
			disabled={disabled || state.loading}
			onclick={() => {
				state.start = 0;
				reader.refresh();
			}}>Search</button>
		<button
			type="button"
			disabled={disabled || state.loading || state.start === 0}
			onclick={() => {
				state.start = Math.max(0, state.start - 64);
				reader.refresh();
			}}>Previous</button>
		<button
			type="button"
			disabled={disabled || state.loading || !state.page || state.start + 64 >= state.page.total}
			onclick={() => {
				state.start += 64;
				reader.refresh();
			}}>Next</button>
	</div>
	{#if state.error}<p role="alert">{state.error}</p>{/if}
</div>
