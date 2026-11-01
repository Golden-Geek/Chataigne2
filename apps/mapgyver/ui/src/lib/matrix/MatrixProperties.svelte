<script lang="ts">
	import { IconButton } from 'golden_ui';
	import deleteIcon from 'golden_ui/style/icons/delete.svg';
	import duplicateIcon from 'golden_ui/style/icons/duplicate.svg';
	import playIcon from 'golden_ui/style/icons/play.svg';
	import arrowIcon from 'golden_ui/style/icons/arrow.svg';
	import referenceIcon from 'golden_ui/style/icons/parameter/reference.svg';
	import searchIcon from 'golden_ui/style/icons/watch.svg';
	import { useMatrixAuthoring } from './context.svelte';
	import { channelKey, channelReferenceFromKey } from '../media/identity';
	import RowForm from './RowForm.svelte';
	import ColumnForm from './ColumnForm.svelte';
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
						placeholder="Find a source…" /><IconButton
						icon={searchIcon}
						label="Search sources"
						type="submit"
						disabled={busy} />
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
					<IconButton
						icon={arrowIcon}
						label="Previous sources"
						class="previous"
						disabled={sources.start === 0 || sources.loading}
						onclick={() => {
							sources.start = Math.max(0, sources.start - 64);
							controller.sources.refresh();
						}} /><IconButton
						icon={arrowIcon}
						label="Next sources"
						disabled={!sources.page || sources.start + 64 >= sources.page.total || sources.loading}
						onclick={() => {
							sources.start += 64;
							controller.sources.refresh();
						}} />
				</div>
				<IconButton
					icon={referenceIcon}
					label="Assign source"
					disabled={busy}
					onclick={() => {
						if (selectedRow?.item.id.kind === 'Layer' && selectedColumn)
							void owner.edit({
								kind: 'SetCells',
								assignments: [
									{
										layer: selectedRow.item.id.id,
										column: selectedColumn.id,
										source: owner.draft ? channelReferenceFromKey(owner.draft) : null
									}
								]
							});
					}} />
			{/if}
			<div class="actions">
				<IconButton
					icon={playIcon}
					label="Launch column"
					disabled={busy || !selectedColumn}
					onclick={() => {
						if (selectedColumn)
							void owner.edit({
								kind: 'LaunchColumn',
								column: selectedColumn.id,
								mode: 'Configured'
							});
					}} /><IconButton
					icon={playIcon}
					label="Cut"
					disabled={busy || !selectedColumn}
					onclick={() => {
						if (selectedColumn)
							void owner.edit({
								kind: 'LaunchColumn',
								column: selectedColumn.id,
								mode: 'Cut'
							});
					}} />
			</div>
			<div class="actions">
				<IconButton
					icon={duplicateIcon}
					label="Copy row"
					disabled={busy || !selectedRow}
					onclick={() => {
						if (selectedRow) void owner.edit({ kind: 'DuplicateItem', item: selectedRow.item.id });
					}} /><IconButton
					icon={duplicateIcon}
					label="Copy column"
					disabled={busy || !selectedColumn}
					onclick={() => {
						if (selectedColumn)
							void owner.edit({ kind: 'DuplicateColumn', column: selectedColumn.id });
					}} />
			</div>
			<div class="actions">
				<IconButton
					icon={deleteIcon}
					label="Remove row"
					disabled={busy || !selectedRow}
					onclick={() => {
						if (selectedRow) void owner.edit({ kind: 'RemoveItem', item: selectedRow.item.id });
					}} /><IconButton
					icon={deleteIcon}
					label="Remove column"
					disabled={busy || !selectedColumn}
					onclick={() => {
						if (selectedColumn)
							void owner.edit({ kind: 'RemoveColumn', column: selectedColumn.id });
					}} />
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
					edit={(value) => owner.edit(value)} />
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
					edit={(value) => owner.edit(value)} />
			{/key}
		{/if}
		<h3>Output preview</h3>
		<RuntimeFeedback snapshot={matrixWindow} {observation} />
		<ChannelPreview channel={matrixWindow.final_output} />
	{/if}
</div>

<style>
	:global(.previous img) {
		transform: rotate(180deg);
	}
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
