<script lang="ts">
	import { untrack } from 'svelte';
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import type { PanelProps, PanelState } from '../../../dockview/panel-types';
	import {
		readPanelPersistedState,
		writePanelPersistedState
	} from '../../../dockview/panel-persistence';
	import { hasDesktopHost, openDesktopFileDialog } from '../../../host/desktop';
	import { beginExplorerFileDrag, endExplorerFileDrag } from '../../../host/file-drop';
	import { browseProjectFiles } from '../../../host/file-explorer';
	import type { FileExplorerCursor } from '../../../generated/file-explorer/FileExplorerCursor';
	import type { FileExplorerEntry } from '../../../generated/file-explorer/FileExplorerEntry';
	import type { FileExplorerPage } from '../../../generated/file-explorer/FileExplorerPage';
	import { projectFileState } from '../../../store/project-files.svelte';
	import { projectFolderParameter } from '../../../store/session/runtime-settings';
	import { sendSetParamIntent } from '../../../store/ui-intents';
	import { appState } from '../../../store/workbench.svelte';
	import IconButton from '../../common/IconButton.svelte';
	import FileExplorerThumbnail from './FileExplorerThumbnail.svelte';
	import folderIcon from '../../../style/icons/node/folder.svg';
	import fileIcon from '../../../style/icons/parameter/file.svg';
	import treeIcon from '../../../style/icons/panels/outliner.svg';
	import tilesIcon from '../../../style/icons/node/dashboard.svg';
	import upIcon from '../../../style/icons/viewport/home.svg';

	type ExplorerView = 'tree' | 'thumbnails';
	let props: PanelProps = $props();
	const persistedView = (params: PanelProps['params']): ExplorerView =>
		readPanelPersistedState<{ view?: ExplorerView }>(params)?.view === 'thumbnails'
			? 'thumbnails'
			: 'tree';
	let view = $state(untrack(() => persistedView(props.params)));
	export const setPanelState = (state: PanelState) => {
		view = persistedView(state.params);
	};
	const desktop = hasDesktopHost();
	const session = $derived(appState.session);
	const setting = $derived(session ? projectFolderParameter(session.graph) : null);
	const folder = $derived(
		setting?.data.kind === 'parameter' && setting.data.param.value.kind === 'file'
			? setting.data.param.value.value || null
			: null
	);
	let root = $state<string | null>(null);
	let rootPending = $state(false);
	let rootError = $state('');
	let picking = $state(false);
	let trail = $state<string[]>([]);
	const pages = new SvelteMap<string, FileExplorerPage>();
	const expanded = new SvelteSet<string>();
	const pending = new SvelteSet<string>();
	const errors = new SvelteMap<string, string>();
	const current = $derived(trail.at(-1) ?? root);
	let epoch = 0;

	$effect(() => {
		const project = projectFileState.currentPath;
		const override = folder;
		const generation = session?.projectGeneration;
		const request = ++epoch;
		untrack(() => {
			root = null;
			rootError = '';
			rootPending = desktop && generation != null && Boolean(project || override);
			pages.clear();
			expanded.clear();
			pending.clear();
			errors.clear();
			trail = [];
			if (!rootPending) return;
			void browseProjectFiles(project, override)
				.then((page) => {
					if (epoch !== request || !page) return;
					pages.set(page.directory, page);
					root = page.root;
				})
				.catch((error) => {
					if (epoch === request) rootError = String(error);
				})
				.finally(() => {
					if (epoch === request) rootPending = false;
				});
		});
		return () => {
			epoch++;
		};
	});

	async function list(directory: string, after?: FileExplorerCursor) {
		const base = root;
		if (!base || pending.has(directory)) return;
		const request = epoch;
		pending.add(directory);
		errors.delete(directory);
		try {
			const page = await browseProjectFiles(null, base, directory, after);
			if (epoch !== request || !page) return;
			const earlier = after ? (pages.get(directory)?.entries ?? []) : [];
			pages.set(directory, { ...page, entries: [...earlier, ...page.entries] });
		} catch (error) {
			if (epoch === request) errors.set(directory, String(error));
		} finally {
			if (epoch === request) pending.delete(directory);
		}
	}

	function expand(path: string) {
		if (expanded.delete(path)) return;
		expanded.add(path);
		if (!pages.has(path)) void list(path);
	}

	function enter(path: string) {
		trail = [...trail, path];
		if (!pages.has(path)) void list(path);
	}

	function chooseView(next: ExplorerView) {
		view = next;
		writePanelPersistedState(props.panelApi, { view });
	}

	function startDrag(event: DragEvent, entry: FileExplorerEntry) {
		const owner = session;
		const generation = owner?.projectGeneration;
		const request = epoch;
		beginExplorerFileDrag(
			event,
			entry.path,
			() => epoch === request && session === owner && owner?.projectGeneration === generation
		);
	}

	async function chooseFolder() {
		const owner = session;
		const field = setting;
		if (!owner || field?.data.kind !== 'parameter' || picking) return;
		const generation = owner.snapshotGeneration;
		picking = true;
		try {
			const value = await openDesktopFileDialog({
				directory: true,
				title: 'Choose the project folder'
			});
			if (
				value &&
				session === owner &&
				owner.snapshotGeneration === generation &&
				setting?.uuid === field.uuid
			)
				await sendSetParamIntent(
					field.node_id,
					{ kind: 'file', value },
					field.data.param.event_behaviour
				);
		} finally {
			picking = false;
		}
	}
</script>

{#snippet entryIcon(entry: FileExplorerEntry)}
	<img class="entry-icon" src={entry.directory ? folderIcon : fileIcon} alt="" draggable="false" />
{/snippet}

{#snippet status(path: string, page: FileExplorerPage | undefined)}
	{#if errors.has(path)}
		<p class="hint" role="alert">{errors.get(path)}</p>
		<button type="button" class="more" onclick={() => void list(path, page?.next ?? undefined)}
			>Retry</button>
	{:else if pending.has(path)}
		<p class="hint" role="status">Reading folder…</p>
	{:else if page?.next}
		<button type="button" class="more" onclick={() => void list(path, page.next ?? undefined)}
			>Load more</button>
	{:else if page?.entries.length === 0}
		<p class="hint">Folder is empty.</p>
	{/if}
{/snippet}

{#snippet tree(path: string)}
	{@const page = pages.get(path)}
	<ul class="tree">
		{#each page?.entries ?? [] as entry (entry.path)}
			<li>
				{#if entry.directory}
					<button
						type="button"
						class="row"
						aria-expanded={expanded.has(entry.path)}
						title={entry.path}
						onclick={() => expand(entry.path)}>
						<span class="disclosure" aria-hidden="true"
							>{expanded.has(entry.path) ? '▾' : '▸'}</span>
						{@render entryIcon(entry)}
						<span class="name">{entry.name}</span>
					</button>
					{#if expanded.has(entry.path)}{@render tree(entry.path)}{/if}
				{:else}
					<div
						class="row draggable"
						role="group"
						aria-roledescription="draggable file"
						draggable="true"
						title={entry.path}
						data-explorer-file={entry.path}
						ondragstart={(event) => startDrag(event, entry)}
						ondragend={endExplorerFileDrag}>
						{@render entryIcon(entry)}
						<span class="name">{entry.name}</span>
					</div>
				{/if}
			</li>
		{/each}
	</ul>
	{@render status(path, page)}
{/snippet}

<section class="file-explorer" aria-label="File Explorer" data-panel-id={props.panelId}>
	<div class="toolbar" role="group" aria-label="File Explorer controls">
		<IconButton
			icon={folderIcon}
			label="Choose project folder"
			disabled={!desktop || !setting || picking || session?.loading.visible}
			onclick={() => void chooseFolder()} />
		<IconButton
			icon={treeIcon}
			label="Tree View"
			aria-pressed={view === 'tree'}
			onclick={() => chooseView('tree')} />
		<IconButton
			icon={tilesIcon}
			label="Thumbnail View"
			aria-pressed={view === 'thumbnails'}
			onclick={() => chooseView('thumbnails')} />
		{#if view === 'thumbnails'}
			<IconButton
				icon={upIcon}
				label="Up one folder"
				disabled={trail.length === 0}
				onclick={() => {
					trail = trail.slice(0, -1);
				}} />
		{/if}
	</div>
	{#if current}
		<p
			class="path"
			aria-label="Current folder"
			title={view === 'tree' ? (root ?? undefined) : current}>
			{view === 'tree' ? root : current}
		</p>
	{/if}
	<div class="entries">
		{#if !desktop}
			<p class="hint">File Explorer is available in the desktop app.</p>
		{:else if rootPending}
			<p class="hint" role="status">Reading folder…</p>
		{:else if rootError}
			<p class="hint" role="alert">{rootError}</p>
		{:else if !root}
			<p class="hint">Save the project or choose a folder.</p>
		{:else if view === 'tree'}
			{@render tree(root)}
		{:else if current}
			{@const page = pages.get(current)}
			<ul class="tiles">
				{#each page?.entries ?? [] as entry (entry.path)}
					<li>
						{#if entry.directory}
							<button
								type="button"
								class="tile"
								title={entry.path}
								onclick={() => enter(entry.path)}>
								<FileExplorerThumbnail {entry} {root} />
								<span class="name">{entry.name}</span>
							</button>
						{:else}
							<div
								class="tile draggable"
								role="group"
								aria-roledescription="draggable file"
								draggable="true"
								title={entry.path}
								data-explorer-file={entry.path}
								ondragstart={(event) => startDrag(event, entry)}
								ondragend={endExplorerFileDrag}>
								<FileExplorerThumbnail {entry} {root} />
								<span class="name">{entry.name}</span>
							</div>
						{/if}
					</li>
				{/each}
			</ul>
			{@render status(current, page)}
		{/if}
	</div>
</section>

<style>
	.file-explorer {
		display: flex;
		flex-direction: column;
		block-size: 100%;
		min-block-size: 0;
		min-inline-size: 0;
	}
	.toolbar {
		display: flex;
		align-items: center;
		gap: 0.15rem;
		padding-inline: 0.35rem;
	}
	.toolbar :global(button[aria-pressed='true']) {
		background: var(--gc-color-selection);
	}
	.path {
		margin: 0.3rem 0.55rem;
		font-size: 0.72rem;
		opacity: 0.72;
	}
	.path,
	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.entries {
		flex: 1;
		overflow: auto;
		padding: 0.2rem 0.4rem;
	}
	.tree,
	.tiles {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.tree .tree {
		padding-inline-start: 0.9rem;
	}
	.row,
	.tile,
	.more {
		color: inherit;
		border: 0;
		border-radius: 0.25rem;
		background: transparent;
		font: inherit;
	}
	.row {
		inline-size: 100%;
		display: flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.18rem 0.3rem;
		text-align: start;
	}
	.row.draggable {
		padding-inline-start: 1.1rem;
	}
	button.row,
	button.tile,
	.more {
		cursor: pointer;
	}
	.row:hover,
	.tile:hover,
	.more:hover {
		background: var(--gc-control-hover-background, #ffffff18);
	}
	.entry-icon {
		inline-size: 0.85rem;
		block-size: 0.85rem;
		object-fit: contain;
	}
	.tiles {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(min(5rem, 100%), 1fr));
		gap: 0.25rem;
	}
	.tile {
		inline-size: 100%;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: 0.3rem;
		text-align: center;
		padding: 0.3rem;
	}
	.draggable {
		cursor: grab;
		user-select: none;
	}
	.hint,
	.more {
		margin: 0.3rem;
		font-size: 0.75rem;
		overflow-wrap: anywhere;
	}
</style>
