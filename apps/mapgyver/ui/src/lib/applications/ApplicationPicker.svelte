<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type { WorkbenchSession } from 'golden_ui';
	import { ApplicationCatalog, catalogState } from './catalog';
	import CatalogControls from './CatalogControls.svelte';
	import { invokeMediaEdit } from '../media/operations';
	import { MEDIA_NAMESPACE } from '../media/generated/constants';
	let {
		session,
		selected,
		exclude = [],
		disabled,
		choose
	}: {
		session: WorkbenchSession | null;
		selected: string | null;
		exclude?: string[];
		disabled: boolean;
		choose(value: string | null): void;
	} = $props();
	const state = $state(catalogState());
	const reader = new ApplicationCatalog(() => session, state, invokeMediaEdit);
	$effect(() => {
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

<div class="picker">
	<label
		>Shared application<select
			value={selected ?? ''}
			{disabled}
			onchange={(event) => choose(event.currentTarget.value || null)}>
			<option value="">Choose an application</option>
			{#if selected && !state.page?.applications.some((app) => app.id === selected)}<option
					value={selected}>Saved reference: {selected}</option
				>{/if}
			{#each state.page?.applications ?? [] as app (app.id)}
				<option value={app.id} disabled={exclude.includes(app.id)}
					>{app.label}{app.enabled ? '' : ' (disabled)'}{app.configured
						? ''
						: ' (unconfigured)'}</option>
			{/each}
		</select></label>
	<CatalogControls
		{state}
		{disabled}
		update={(search, start, refresh) => {
			state.search = search;
			state.start = start;
			if (refresh) reader.refresh();
		}} />
</div>

<style>
	.picker,
	label {
		display: grid;
		gap: 0.4rem;
		min-width: 0;
	}
	select {
		min-width: 0;
		max-width: 100%;
	}
</style>
