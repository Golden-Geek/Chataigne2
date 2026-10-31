<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { appState, type PanelProps, type PanelState, type WorkbenchSession } from 'golden_ui';
	import { MatrixController, matrixState } from './authoring';
	import { choiceState } from './choices';
	import { invokeMediaEdit } from '../media/operations';
	import { channelKey, channelReferenceFromKey } from '../media/identity';
	import { MEDIA_NAMESPACE, MATRIX_PLAYBACK_EVENT } from '../media/generated/constants';
	import ChannelPreview from '../media/ChannelPreview.svelte';
	import VirtualGrid from './VirtualGrid.svelte';
	import RowForm from './RowForm.svelte';
	import ColumnForm from './ColumnForm.svelte';
	import MatrixForm from './MatrixForm.svelte';
	import './forms.css';
	let { panelId, panelType, title }: PanelProps = $props();
	let panel = $state<PanelState | null>(null);
	export const setPanelState = (next: PanelState) => {
		panel = next;
	};
	const session = $derived(appState.session);
	const view = $state(matrixState()),
		matrices = $state(choiceState()),
		sources = $state(choiceState());
	const controller = new MatrixController<WorkbenchSession>(
		() => session,
		view,
		matrices,
		sources,
		invokeMediaEdit
	);
	const busy = $derived(controller.busy || session?.hasActiveEditSession === true);
	const selectedRow = $derived(
		view.window?.rows.find((row) => row.item.id.id === view.selected?.item?.id)
	);
	const selectedColumn = $derived(
		view.window?.columns.find((column) => column.id === view.selected?.column)
	);
	const selectedCell = $derived(
		view.window?.cells.find(
			(value) =>
				value.cell.layer === view.selected?.item?.id &&
				value.cell.definition.column === view.selected?.column
		)
	);
	let draft = $state('');
	let draftKey = '';
	const definition = $derived.by(() => {
		const source = view.window?.final_output.definition.source;
		return source?.kind === 'Matrix' && source.output.kind === 'Final'
			? source.output.definition
			: null;
	});
	$effect(() => {
		const source = selectedCell?.cell.definition.source;
		const key = JSON.stringify([view.media, view.selected, source]);
		untrack(() => {
			if (key !== draftKey) {
				draftKey = key;
				draft = source ? channelKey(source) : '';
			}
		});
	});
	$effect(() => {
		const current = session;
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
			if (!current?.hasActiveEditSession) controller.observe();
		});
	});
	$effect(() => {
		const current = session;
		if (current) void current.getCustomEventSequence(MATRIX_PLAYBACK_EVENT);
		untrack(() => {
			if (!current?.hasActiveEditSession) controller.observe(false);
		});
	});
	onDestroy(() => controller.dispose());
</script>

<section
	class="matrix"
	aria-label={panel?.title ?? title ?? 'Matrix'}
	data-panel-id={panelId}
	data-panel-type={panelType}>
	<header>
		<h2>Matrix</h2>
		<button disabled={busy} onclick={() => controller.create()}>Add Matrix</button><button
			disabled={busy}
			onclick={() => controller.observe()}>Refresh</button>
	</header>
	{#if view.error || view.readError}<p role="alert">{view.error || view.readError}</p>{/if}
	{#if matrices.error}<p role="alert">{matrices.error}</p>{/if}
	<form
		class="choices"
		onsubmit={(event) => {
			event.preventDefault();
			matrices.start = 0;
			controller.matrices.refresh();
		}}>
		<input
			aria-label="Find Matrix"
			type="search"
			bind:value={matrices.search}
			placeholder="Find a Matrix…" /><button disabled={busy}>Search</button>
		<select
			aria-label="Matrix media"
			disabled={busy}
			value={view.media ?? ''}
			onchange={(event) => controller.selectMedia(event.currentTarget.value || null)}>
			<option value="">Choose Matrix</option>
			{#if view.media && !matrices.page?.choices.some((choice) => choice.reference.media === view.media)}<option
					value={view.media}>Selected Matrix</option
				>{/if}
			{#each matrices.page?.choices ?? [] as choice (choice.reference.channel)}<option
					value={choice.reference.media}>{choice.label}</option
				>{/each}
		</select>
		<button
			type="button"
			disabled={matrices.start === 0 || matrices.loading}
			onclick={() => {
				matrices.start = Math.max(0, matrices.start - 64);
				controller.matrices.refresh();
			}}>Previous</button>
		<button
			type="button"
			disabled={!matrices.page || matrices.start + 64 >= matrices.page.total || matrices.loading}
			onclick={() => {
				matrices.start += 64;
				controller.matrices.refresh();
			}}>Next</button>
	</form>
	{#if view.window}
		<div class="actions">
			<button
				disabled={busy}
				onclick={() => controller.edit({ kind: 'AddLayers', parent: null, count: 1 })}
				>Add layer</button>
			<button
				disabled={busy}
				onclick={() => controller.edit({ kind: 'AddGroup', parent: null, label: null })}
				>Add group</button>
			<button disabled={busy} onclick={() => controller.edit({ kind: 'AddColumns', count: 1 })}
				>Add column</button>
			<span>{view.window.row_total} rows · {view.window.column_total} columns</span>
			{#if view.refreshing}<span role="status">Refreshing visible cells…</span>{/if}
		</div>
		<div class="workspace">
			{#key view.media}
				<VirtualGrid
					snapshot={view.window}
					selected={view.selected}
					disabled={busy}
					viewport={(value) => controller.setViewport(value)}
					selectRow={(row) => controller.select(row.item.id, view.selected?.column ?? null)}
					selectColumn={(column) => controller.select(view.selected?.item ?? null, column.id)}
					select={(row, column) => {
						controller.select(row.item.id, column.id);
						const source = view.window?.cells.find(
							(value) =>
								value.cell.layer === row.item.id.id && value.cell.definition.column === column.id
						)?.cell.definition.source;
						draft = source ? channelKey(source) : '';
					}}
					launch={(column) =>
						controller.edit({ kind: 'LaunchColumn', column, mode: 'Configured' })} />
			{/key}
			<aside>
				<h3>Selection and assignment</h3>
				{#if selectedRow || selectedColumn}
					<p>{selectedRow?.item.label ?? 'No row'} / {selectedColumn?.label ?? 'No column'}</p>
					{#if selectedRow?.item.id.kind === 'Layer' && selectedColumn}
						<p>
							Assigned source: {selectedCell?.source_missing
								? 'Missing source'
								: (selectedCell?.source_label ?? 'Empty')}
						</p>
						<form
							onsubmit={(event) => {
								event.preventDefault();
								sources.start = 0;
								controller.sources.refresh();
							}}>
							<input
								type="search"
								aria-label="Find Matrix source"
								bind:value={sources.search}
								placeholder="Find a source…" /><button disabled={busy}>Search sources</button>
						</form>
						{#if sources.error}<p role="alert">{sources.error}</p>{/if}
						<label
							>Source to assign<select bind:value={draft} disabled={busy}>
								<option value="">Empty</option>
								{#if draft && !sources.page?.choices.some((choice) => channelKey(choice.reference) === draft)}<option
										value={draft}>Selected source</option
									>{/if}
								{#each sources.page?.choices ?? [] as choice (choice.reference.channel)}<option
										value={channelKey(choice.reference)}>{choice.label}</option
									>{/each}
							</select></label>
						<div class="actions">
							<button
								disabled={sources.start === 0 || sources.loading}
								onclick={() => {
									sources.start = Math.max(0, sources.start - 64);
									controller.sources.refresh();
								}}>Previous sources</button
							><button
								disabled={!sources.page ||
									sources.start + 64 >= sources.page.total ||
									sources.loading}
								onclick={() => {
									sources.start += 64;
									controller.sources.refresh();
								}}>Next sources</button>
						</div>
						<button
							disabled={busy}
							onclick={() => {
								if (selectedRow?.item.id.kind === 'Layer' && selectedColumn)
									void controller.edit({
										kind: 'SetCells',
										assignments: [
											{
												layer: selectedRow.item.id.id,
												column: selectedColumn.id,
												source: draft ? channelReferenceFromKey(draft) : null
											}
										]
									});
							}}>Assign source</button>
					{/if}
					<div class="actions">
						<button
							disabled={busy || !selectedColumn}
							onclick={() => {
								if (selectedColumn)
									void controller.edit({
										kind: 'LaunchColumn',
										column: selectedColumn.id,
										mode: 'Configured'
									});
							}}>Launch column</button
						><button
							disabled={busy || !selectedColumn}
							onclick={() => {
								if (selectedColumn)
									void controller.edit({
										kind: 'LaunchColumn',
										column: selectedColumn.id,
										mode: 'Cut'
									});
							}}>Cut</button>
					</div>
					<div class="actions">
						<button
							disabled={busy || !selectedRow}
							onclick={() => {
								if (selectedRow)
									void controller.edit({ kind: 'DuplicateItem', item: selectedRow.item.id });
							}}>Copy row</button
						><button
							disabled={busy || !selectedColumn}
							onclick={() => {
								if (selectedColumn)
									void controller.edit({ kind: 'DuplicateColumn', column: selectedColumn.id });
							}}>Copy column</button>
					</div>
					<div class="actions">
						<button
							disabled={busy || !selectedRow}
							onclick={() => {
								if (selectedRow)
									void controller.edit({ kind: 'RemoveItem', item: selectedRow.item.id });
							}}>Remove row</button
						><button
							disabled={busy || !selectedColumn}
							onclick={() => {
								if (selectedColumn)
									void controller.edit({ kind: 'RemoveColumn', column: selectedColumn.id });
							}}>Remove column</button>
					</div>
				{:else}<p>
						Select a row, column or cell. Selection and source assignment do not launch it.
					</p>{/if}
				{#if selectedRow && view.media}
					{#key JSON.stringify(selectedRow.item)}
						<RowForm
							row={selectedRow}
							media={view.media}
							{session}
							disabled={busy}
							edit={(value) => controller.edit(value)} />
					{/key}
				{/if}
				{#if selectedColumn && view.media && definition}
					{#key JSON.stringify([selectedColumn, definition.transition])}
						<ColumnForm
							column={selectedColumn}
							transition={definition.transition}
							media={view.media}
							{session}
							disabled={busy}
							edit={(value) => controller.edit(value)} />
					{/key}
				{/if}
				{#if definition}
					{#key JSON.stringify(definition)}
						<MatrixForm {definition} disabled={busy} edit={(value) => controller.edit(value)} />
					{/key}
				{/if}
				<h3>Output preview</h3>
				<p>
					Requested column: {view.window.playback.desired_column
						? (view.window.columns.find(
								(column) => column.id === view.window?.playback.desired_column
							)?.label ?? 'Outside visible columns')
						: 'None'}
				</p>
				<ChannelPreview channel={view.window.final_output} />
			</aside>
		</div>
	{:else if view.media}<p>Loading Matrix viewport…</p>{:else}<p>Add or choose a Matrix.</p>{/if}
</section>

<style>
	.matrix {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
		gap: 0.5rem;
		padding: 0.75rem;
		box-sizing: border-box;
		background: var(--mapgyver-panel-background, #191b20);
		color: inherit;
	}
	header,
	.choices,
	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.4rem;
	}
	h2 {
		margin: 0;
		margin-right: auto;
		font-size: 1.1rem;
	}
	.workspace {
		display: flex;
		flex: 1;
		min-height: 0;
		gap: 0.75rem;
	}
	aside {
		width: 19rem;
		min-width: 12rem;
		max-width: 40%;
		overflow: auto;
	}
	input,
	select {
		max-width: 100%;
		min-width: 0;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}
	h3 {
		font-size: 1rem;
	}
	p {
		font-size: 0.85rem;
	}
</style>
