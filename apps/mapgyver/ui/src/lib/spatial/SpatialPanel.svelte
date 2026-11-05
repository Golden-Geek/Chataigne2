<script lang="ts">
	import { untrack } from 'svelte';
	import {
		appState,
		readPanelPersistedState,
		writePanelPersistedState,
		type PanelProps,
		type PanelState
	} from 'golden_ui';
	import { spatialPanelState, sameTarget, targetKey, type SpatialPanelState } from './targets';
	import { createSpatialBinding } from './binding.svelte';
	import { followPanelTitle } from '../panel-title.svelte';
	import CompositionAdapter from '../composition/CompositionAdapter.svelte';
	import MatrixLayoutAdapter from './MatrixLayoutAdapter.svelte';
	import MatrixCellLayoutAdapter from './MatrixCellLayoutAdapter.svelte';
	let props: PanelProps = $props();
	let choice = $state(
		untrack(() => spatialPanelState(readPanelPersistedState<SpatialPanelState>(props.params)))
	);
	let layerLabel = $state<string | null>(null);
	export const setPanelState = (next: PanelState) => {
		choice = spatialPanelState(readPanelPersistedState<SpatialPanelState>(next.params));
	};
	const owner = createSpatialBinding(() => ({
		selected: appState.session?.getSelectedNodes()[0]?.uuid ?? null,
		document: choice.document,
		locked: choice.locked,
		retained: choice.retained
	}));
	const binding = $derived(owner.binding),
		target = $derived(binding?.target ?? null);
	const page = $derived(sameTarget(choice.retained, target) ? choice.page : 0);
	const matrixScope = $derived(
		target?.kind === 'Matrix' && choice.matrix?.media === target.media ? choice.matrix : null
	);
	function setMatrix(matrix: SpatialPanelState['matrix']) {
		choice.matrix = matrix;
		persist();
	}
	function persist() {
		writePanelPersistedState(props.panelApi, { ...choice });
	}
	function setPage(page: number) {
		choice.page = page;
		persist();
	}
	$effect(() => {
		if (!binding?.present || !binding.document_matches || !target) return;
		if (choice.document === binding.document && sameTarget(choice.retained, target)) return;
		choice = {
			...choice,
			document: binding.document,
			retained: target,
			page: 0,
			matrix: choice.document === binding.document ? choice.matrix : null
		};
		untrack(persist);
	});
	$effect(() =>
		props.panelApi.setContentBinding({
			locked: !!choice.locked,
			canLock: binding?.present === true,
			toggle: () => {
				choice.locked = choice.locked ? null : target;
				persist();
			}
		})
	);
	followPanelTitle(
		() => props.panelApi,
		() => 'Composition',
		() =>
			binding?.present
				? `${binding.label}${target?.kind === 'Matrix' ? (matrixScope ? ` / ${layerLabel ? `${layerLabel} ` : ''}Cells` : ' / Matrix Layers') : ''}`
				: null
	);
</script>

{#if binding?.document_matches && target}
	{#key targetKey(target)}
		{#if target.kind === 'Composition'}
			<CompositionAdapter
				panelId={props.panelId}
				title={props.title ?? 'Composition'}
				reference={target.reference}
				document={binding.document}
				present={binding.present}
				locked={!!choice.locked}
				{page}
				onpagechange={setPage} />
		{:else if matrixScope}
			{#key matrixScope.layer}
				<MatrixCellLayoutAdapter
					panelId={props.panelId}
					media={target.media}
					layer={matrixScope.layer}
					present={binding.present}
					locked={!!choice.locked}
					page={matrixScope.page}
					cell={matrixScope.cell}
					onback={() => setMatrix(null)}
					onpage={(page) => setMatrix({ ...matrixScope, page })}
					oncell={(cell) => setMatrix({ ...matrixScope, cell })}
					onlabel={(label) => {
						layerLabel = label;
					}} />
			{/key}
		{:else}
			<MatrixLayoutAdapter
				panelId={props.panelId}
				media={target.media}
				document={binding.document}
				present={binding.present}
				locked={!!choice.locked}
				{page}
				onpagechange={setPage}
				onenterlayer={(layer) => setMatrix({ media: target.media, layer, cell: null, page: 0 })} />
		{/if}
	{/key}
{:else}
	<section
		class="workbench-editor"
		aria-label={props.title ?? 'Composition'}
		data-panel-id={props.panelId}
		data-spatial-kind={choice.locked?.kind ?? ''}
		data-spatial-media=""
		data-composition-channel=""
		data-composition-locked={!!choice.locked}>
		<div class="editor-canvas">
			{#if owner.state.error}<p class="editor-error" role="alert">{owner.state.error}</p>
			{:else if choice.locked}<p class="editor-empty">
					Locked {choice.locked.kind} unavailable in this project.
				</p>
			{:else if owner.state.refreshing}<p class="editor-empty" role="status">Loading layout…</p>
			{:else}<p class="editor-empty">Select a Composition or Matrix in Medias.</p>{/if}
		</div>
	</section>
{/if}
