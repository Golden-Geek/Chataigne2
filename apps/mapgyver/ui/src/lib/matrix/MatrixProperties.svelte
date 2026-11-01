<script lang="ts">
	import { useMatrixAuthoring } from './context.svelte';
	import { channelKey, channelReferenceFromKey } from '../media/identity';
	import RowForm from './RowForm.svelte';
	import ColumnForm from './ColumnForm.svelte';
	import MatrixForm from './MatrixForm.svelte';
	import ChannelPreview from '../media/ChannelPreview.svelte';
	import RuntimeFeedback from './RuntimeFeedback.svelte';
	import './forms.css';
	const owner = useMatrixAuthoring();
	const { view, controller, matrices, sources } = owner;
	const session = $derived(owner.session),
		busy = $derived(owner.busy),
		matrixWindow = $derived(owner.matrixWindow),
		observation = $derived(owner.observation);
	const selectedRow = $derived(owner.selectedRow),
		selectedColumn = $derived(owner.selectedColumn),
		selectedCell = $derived(owner.selectedCell),
		definition = $derived(owner.definition);
</script>

<div class="properties">
	{#if matrixWindow}
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
					>Source to assign<select bind:value={owner.draft} disabled={busy}>
						<option value="">Empty</option>
						{#if owner.draft && !sources.page?.choices.some((choice) => channelKey(choice.reference) === owner.draft)}<option
								value={owner.draft}>Selected source</option
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
						disabled={!sources.page || sources.start + 64 >= sources.page.total || sources.loading}
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
										source: owner.draft ? channelReferenceFromKey(owner.draft) : null
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
		<RuntimeFeedback snapshot={matrixWindow} {observation} />
		<ChannelPreview channel={matrixWindow.final_output} />
	{/if}
</div>

<style>
	.properties {
		display: grid;
		gap: 0.5rem;
		padding: 0.5rem;
		min-width: 0;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.25rem;
	}
	h3 {
		font-size: 0.9rem;
		margin: 0.25rem 0;
	}
	p {
		font-size: 0.8rem;
		margin: 0.25rem 0;
	}
</style>
