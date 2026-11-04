<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import {
		appState,
		IconButton,
		readPanelPersistedState,
		writePanelPersistedState,
		type PanelProps,
		type PanelState
	} from 'golden_ui';
	import lockIcon from 'golden_ui/style/icons/lock.svg';
	import viewIcon from 'golden_ui/style/icons/viewport/frame.svg';
	import arrowIcon from 'golden_ui/style/icons/arrow.svg';
	import ChannelPreview from './ChannelPreview.svelte';
	import { invokeMediaEdit } from './operations';
	import { MEDIA_NAMESPACE } from './generated/constants';
	import { MediaPreviewReader, previewReadState } from './preview-reader';
	import { previewState, PREVIEW_PAGE_SIZE, type MediaPreviewState } from './preview-state';
	let props: PanelProps = $props();
	let choice = $state(
		untrack(() => previewState(readPanelPersistedState<MediaPreviewState>(props.params)))
	);
	let title = $state(untrack(() => props.title));
	export const setPanelState = (next: PanelState) => {
		title = next.title;
		choice = previewState(readPanelPersistedState<MediaPreviewState>(next.params));
	};
	const session = $derived(appState.session);
	const selected = $derived(session?.getSelectedNodes()[0]?.uuid ?? null);
	const readState = $state(previewReadState());
	const reader = new MediaPreviewReader(() => session, readState, invokeMediaEdit);
	const page = $derived(reader.page);
	const channels = $derived(page?.channels?.channels ?? []);
	const current = $derived(page?.selected ?? null);
	const pages = $derived(Math.max(1, Math.ceil((page?.channels?.total ?? 0) / PREVIEW_PAGE_SIZE)));
	function persist() {
		writePanelPersistedState(props.panelApi, { ...choice });
	}
	$effect(() => {
		const current = session;
		const query = {
			selected: choice.lockedMedia ?? selected,
			document: choice.lockedMedia ? choice.document : null,
			channel: choice.channel,
			start: choice.page * PREVIEW_PAGE_SIZE,
			count: PREVIEW_PAGE_SIZE
		};
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.projectGeneration;
			void current.currentHistoryStateId;
			void current.hasActiveEditSession;
			void current.editSessionEpoch;
			void current.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		untrack(() => reader.refresh(query));
	});
	$effect(() => {
		if (page?.channels && choice.page >= pages) {
			choice.page = pages - 1;
			untrack(persist);
		}
	});
	onDestroy(() => reader.dispose());
</script>

<section
	class="workbench-editor media-preview-panel"
	aria-label={title}
	data-panel-id={props.panelId}
	data-preview-media={page?.media ?? ''}
	data-preview-locked={!!choice.lockedMedia}>
	<div class="editor-toolbar">
		<span class="media-label">{page?.label ?? 'Media Preview'}</span>
		<IconButton
			icon={lockIcon}
			label={choice.lockedMedia ? 'Unlock media' : 'Lock media'}
			disabled={!choice.lockedMedia && !page?.media}
			onclick={() => {
				if (choice.lockedMedia) {
					choice.lockedMedia = null;
					choice.document = null;
				} else if (page?.media) {
					choice.lockedMedia = page.media;
					choice.document = page.document;
				}
				persist();
			}} />
		<IconButton
			icon={viewIcon}
			label={choice.grid ? 'Single channel' : 'All-channel grid'}
			onclick={() => {
				choice.grid = !choice.grid;
				persist();
			}} />
		{#if !choice.grid && current}
			<select
				aria-label="Preview channel"
				value={current.reference.channel}
				onchange={(event) => {
					choice.channel = event.currentTarget.value;
					persist();
				}}>
				{#if !channels.some((channel) => channel.reference.channel === current.reference.channel)}<option
						value={current.reference.channel}>{current.label}</option
					>{/if}
				{#each channels as channel (channel.reference.channel)}<option
						value={channel.reference.channel}>{channel.label}</option
					>{/each}
			</select>
		{/if}
		{#if pages > 1}
			<IconButton
				icon={arrowIcon}
				label="Previous preview channels"
				class="previous"
				disabled={choice.page === 0 || readState.loading}
				onclick={() => {
					choice.page--;
					persist();
				}} />
			<span>{choice.page + 1}/{pages}</span>
			<IconButton
				icon={arrowIcon}
				label="Next preview channels"
				disabled={choice.page + 1 >= pages || readState.loading}
				onclick={() => {
					choice.page++;
					persist();
				}} />
		{/if}
	</div>
	<div class="preview-area">
		{#if readState.error}<p class="editor-error" role="alert">{readState.error}</p>
		{:else if page && !page.document_matches}<p class="editor-empty">
				Locked media belongs to another project.
			</p>
		{:else if !page?.media}<p class="editor-empty">
				{readState.loading
					? 'Loading media…'
					: choice.lockedMedia
						? 'Locked media unavailable.'
						: 'Select media or one of its children.'}
			</p>
		{:else if choice.grid && channels.length}
			<div
				class="channel-grid"
				aria-label="All-channel grid"
				style:grid-template-columns={channels.length > 1 ? 'repeat(2,minmax(0,1fr))' : '1fr'}>
				<!-- A viewport slot keeps its native view while its channel source is replaced. -->
				{#each channels as channel, index (index)}<figure>
						<figcaption>{channel.label}</figcaption>
						<div class="image"><ChannelPreview {channel} fill /></div>
					</figure>{/each}
			</div>
		{:else if !choice.grid && current}<ChannelPreview channel={current} fill />
		{:else}<p class="editor-empty">
				{page.channels?.status === 'MissingTarget'
					? 'Target unavailable.'
					: page.channels?.status === 'Unconfigured'
						? 'Choose a target in the Inspector.'
						: 'No channels.'}
			</p>{/if}
	</div>
</section>

<style>
	.media-label {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	select {
		max-width: 40%;
		min-width: 0;
		background: var(--gc-input-background, transparent);
		color: inherit;
		border: 1px solid var(--gc-border-color, #555);
		border-radius: 0.2rem;
	}
	.preview-area {
		flex: 1;
		min-height: 0;
		overflow: hidden;
	}
	.channel-grid {
		display: grid;
		grid-auto-rows: minmax(0, 1fr);
		height: 100%;
		gap: 0.5rem;
		padding: 0.5rem;
		box-sizing: border-box;
	}
	figure {
		margin: 0;
		min-height: 0;
		min-width: 0;
		display: flex;
		flex-direction: column;
	}
	figcaption {
		font-size: 0.8em;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.image {
		flex: 1;
		min-height: 0;
	}
	:global(.media-preview-panel .previous img) {
		transform: rotate(180deg);
	}
</style>
