<script lang="ts">
	import type { PanelProps, PanelState } from '../../../dockview/panel-types';
	import type { NodeId, ParamValue, UiNodeDto } from '../../../types';
	import { appState } from '../../../store/workbench.svelte';
	import {
		sendCreateUserItemByTypeIntent,
		sendRemoveNodeIntent,
		sendSetParamIntent
	} from '../../../store/ui-intents';

	type EditorParams = { moduleNodeId?: NodeId };

	const MODULE_TYPE = 'streamdeck_module';
	const PAGE_HOST_TYPE = 'paging_page_host';

	// rows x cols per model id.
	const MODEL_GRID: Record<string, [number, number]> = {
		mini: [2, 3],
		standard: [3, 5],
		xl: [4, 8],
		plus: [2, 4],
		pedal: [1, 3]
	};

	let { panelApi, panelId, panelType, title, params }: PanelProps = $props();

	let panel = $state<PanelState>({ panelId: '', panelType: '', title: '', params: {} });
	export const setPanelState = (next: PanelState): void => {
		panel = next;
	};
	$effect(() => {
		panel = { panelId, panelType, title, params };
	});

	let session = $derived(appState.session);
	let graph = $derived(session?.graph.state ?? null);
	let panelParams = $derived((panel.params ?? {}) as EditorParams);

	let simulateMode = $state(false);
	let editingTextKey = $state<NodeId | null>(null);

	const autofocusInput = (node: HTMLInputElement): void => {
		node.focus();
		node.select();
	};

	// --- graph helpers -------------------------------------------------------

	const declLast = (declId: string): string => {
		const slash = declId.lastIndexOf('/');
		return slash >= 0 ? declId.slice(slash + 1) : declId;
	};

	// The backend keys pages by their stable `short_name` (what `active_page` enum options use),
	// not `decl_id` (which keeps the original label-derived form for user-created pages).
	const pageIdOf = (node: UiNodeDto): string => {
		const shortName = node.meta.short_name?.trim();
		return shortName && shortName.length > 0 ? shortName : declLast(node.decl_id);
	};

	const childByKey = (node: UiNodeDto | null, key: string): UiNodeDto | null => {
		if (!graph || !node) {
			return null;
		}
		for (const childId of node.children) {
			const child = graph.nodesById.get(childId);
			if (!child) {
				continue;
			}
			if (child.decl_id === key || declLast(child.decl_id) === key || child.meta.label === key) {
				return child;
			}
		}
		return null;
	};

	const childByType = (node: UiNodeDto | null, nodeType: string): UiNodeDto | null => {
		if (!graph || !node) {
			return null;
		}
		for (const childId of node.children) {
			const child = graph.nodesById.get(childId);
			if (child && child.node_type === nodeType) {
				return child;
			}
		}
		return null;
	};

	const folderChildren = (node: UiNodeDto | null): UiNodeDto[] => {
		if (!graph || !node) {
			return [];
		}
		const out: UiNodeDto[] = [];
		for (const childId of node.children) {
			const child = graph.nodesById.get(childId);
			if (child && child.data.kind === 'node') {
				out.push(child);
			}
		}
		return out;
	};

	const paramChildren = (node: UiNodeDto | null): UiNodeDto[] => {
		if (!graph || !node) {
			return [];
		}
		const out: UiNodeDto[] = [];
		for (const childId of node.children) {
			const child = graph.nodesById.get(childId);
			if (child && child.data.kind === 'parameter') {
				out.push(child);
			}
		}
		return out;
	};

	const enumValueOf = (node: UiNodeDto | null): string => {
		if (node?.data.kind === 'parameter' && node.data.param.value.kind === 'enum') {
			return node.data.param.value.value;
		}
		return '';
	};

	const setParam = (node: UiNodeDto | null | undefined, value: ParamValue): void => {
		if (!node || node.data.kind !== 'parameter') {
			return;
		}
		void sendSetParamIntent(node.node_id, value, node.data.param.event_behaviour);
	};

	// --- module / page resolution -------------------------------------------

	let modules = $derived.by((): UiNodeDto[] => {
		if (!graph) {
			return [];
		}
		return [...graph.nodesById.values()]
			.filter((candidate) => candidate.node_type === MODULE_TYPE)
			.sort((a, b) => a.meta.label.localeCompare(b.meta.label));
	});

	let activeModule = $derived.by((): UiNodeDto | null => {
		if (modules.length === 0) {
			return null;
		}
		return modules.find((m) => m.node_id === panelParams.moduleNodeId) ?? modules[0];
	});

	let parametersFolder = $derived(childByKey(activeModule, 'parameters'));
	let valuesFolder = $derived(childByKey(activeModule, 'values'));
	let modelParam = $derived(childByKey(parametersFolder, 'model'));
	let activePageParam = $derived(childByKey(parametersFolder, 'active_page'));
	let controlKeysFolder = $derived(childByKey(parametersFolder, 'keys'));
	let pageHost = $derived(childByType(parametersFolder, PAGE_HOST_TYPE));

	let modelId = $derived(enumValueOf(modelParam) || 'standard');
	let grid = $derived(MODEL_GRID[modelId] ?? MODEL_GRID.standard);

	type PageEntry = {
		id: string;
		label: string;
		controlRoot: UiNodeDto;
		pageNode: UiNodeDto | null;
	};

	let pages = $derived.by((): PageEntry[] => {
		const out: PageEntry[] = [];
		if (controlKeysFolder) {
			out.push({ id: 'default', label: 'Default', controlRoot: controlKeysFolder, pageNode: null });
		}
		for (const page of folderChildren(pageHost)) {
			out.push({
				id: pageIdOf(page),
				label: page.meta.label,
				controlRoot: page,
				pageNode: page
			});
		}
		return out;
	});

	let activePageId = $derived(enumValueOf(activePageParam) || 'default');
	let activePage = $derived(pages.find((p) => p.id === activePageId) ?? pages[0] ?? null);

	let activeValueRoot = $derived.by((): UiNodeDto | null => {
		if (!valuesFolder) {
			return null;
		}
		if (!activePage || activePage.id === 'default') {
			return childByKey(valuesFolder, 'keys');
		}
		const container =
			childByType(valuesFolder, PAGE_HOST_TYPE) ?? childByKey(valuesFolder, 'pages');
		for (const page of folderChildren(container)) {
			if (pageIdOf(page) === activePage.id) {
				return page;
			}
		}
		return childByKey(valuesFolder, 'keys');
	});

	let defaultKeyFolders = $derived(folderChildren(controlKeysFolder));

	type KeyCell = {
		slot: number;
		folder: UiNodeDto;
		colorNode: UiNodeDto | null;
		textNode: UiNodeDto | null;
		imageNode: UiNodeDto | null;
		valueNode: UiNodeDto | null;
		color: string;
		text: string;
		image: string;
		pressed: boolean;
		permanent: boolean;
	};

	let keys = $derived.by((): KeyCell[] => {
		if (!activePage) {
			return [];
		}
		const activeFolders = folderChildren(activePage.controlRoot);
		const valueParams = paramChildren(activeValueRoot);
		const isDefaultPage = activePage.id === 'default';
		return activeFolders.map((activeFolder, slot) => {
			const defaultFolder = defaultKeyFolders[slot] ?? activeFolder;
			// `Unpaged` is the 4th field and lives only on the default key.
			const defaultUnpaged = boolValue(paramChildren(defaultFolder)[3] ?? null);
			// An un-paged key is permanent: on non-default pages it shows the default appearance
			// and is edited on the default page (so its per-page version is not exposed here).
			const permanent = !isDefaultPage && defaultUnpaged;
			const sourceFolder = permanent ? defaultFolder : activeFolder;
			const fields = paramChildren(sourceFolder);
			const valueNode = valueParams[slot] ?? null;
			return {
				slot,
				folder: sourceFolder,
				colorNode: fields[0] ?? null,
				textNode: fields[1] ?? null,
				imageNode: fields[2] ?? null,
				valueNode,
				color: colorCss(fields[0] ?? null),
				text: strValue(fields[1] ?? null),
				image: fileValue(fields[2] ?? null),
				pressed: boolValue(valueNode),
				permanent
			};
		});
	});

	const colorCss = (node: UiNodeDto | null): string => {
		if (node?.data.kind === 'parameter' && node.data.param.value.kind === 'color') {
			const [r, g, b, a] = node.data.param.value.value;
			return `rgba(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)}, ${a})`;
		}
		return '#1a1a1a';
	};
	const strValue = (node: UiNodeDto | null): string =>
		node?.data.kind === 'parameter' && node.data.param.value.kind === 'str'
			? node.data.param.value.value
			: '';
	const fileValue = (node: UiNodeDto | null): string =>
		node?.data.kind === 'parameter' && node.data.param.value.kind === 'file'
			? node.data.param.value.value
			: '';
	const boolValue = (node: UiNodeDto | null): boolean =>
		node?.data.kind === 'parameter' && node.data.param.value.kind === 'bool'
			? node.data.param.value.value
			: false;

	// --- actions -------------------------------------------------------------

	const selectModule = (id: NodeId): void => {
		panelApi.updateParams({ ...panelParams, moduleNodeId: id });
	};

	const selectPage = (id: string): void => {
		setParam(activePageParam, { kind: 'enum', value: id });
	};

	const addPage = (): void => {
		if (pageHost) {
			void sendCreateUserItemByTypeIntent(pageHost.node_id, 'folder', 'New Page');
		}
	};

	const deleteActivePage = (): void => {
		if (activePage?.pageNode) {
			void sendRemoveNodeIntent(activePage.pageNode.node_id);
		}
	};

	const selectKey = (cell: KeyCell): void => {
		session?.selectNode(cell.folder.node_id, 'REPLACE');
	};

	const pressKey = (cell: KeyCell): void => {
		setParam(cell.valueNode, { kind: 'bool', value: true });
	};
	const releaseKey = (cell: KeyCell): void => {
		setParam(cell.valueNode, { kind: 'bool', value: false });
	};

	const onKeyPointerDown = (cell: KeyCell): void => {
		if (simulateMode) {
			pressKey(cell);
		} else {
			selectKey(cell);
		}
	};
	const onKeyPointerUp = (cell: KeyCell): void => {
		if (simulateMode) {
			releaseKey(cell);
		}
	};

	const commitText = (cell: KeyCell, text: string): void => {
		editingTextKey = null;
		if (text !== cell.text) {
			setParam(cell.textNode, { kind: 'str', value: text });
		}
	};

	const onDropImage = (cell: KeyCell, event: DragEvent): void => {
		event.preventDefault();
		if (cell.permanent) {
			return; // permanent (un-paged) keys are edited on the default page
		}
		const file = event.dataTransfer?.files?.[0];
		if (!file) {
			return;
		}
		// Tauri/Electron expose an absolute path on dropped File objects; fall back to the name.
		const path = (file as unknown as { path?: string }).path ?? file.name;
		setParam(cell.imageNode, { kind: 'file', value: path });
	};

	// The slot of the currently selected key (searched across all pages of this module).
	let selectedSlot = $derived.by((): number | null => {
		const selected = session?.selectedNodeId;
		if (selected === null || selected === undefined) {
			return null;
		}
		for (const page of pages) {
			const index = folderChildren(page.controlRoot).findIndex((f) => f.node_id === selected);
			if (index >= 0) {
				return index;
			}
		}
		return null;
	});

	// On a page switch, keep the same key slot selected (select this page's version of it).
	let lastActivePageId = $state('');
	$effect(() => {
		const pageId = activePageId;
		if (pageId === lastActivePageId) {
			return;
		}
		const isFirstRun = lastActivePageId === '';
		lastActivePageId = pageId;
		if (isFirstRun) {
			return;
		}
		const slot = selectedSlot;
		if (slot === null) {
			return;
		}
		const target = keys[slot];
		if (target && session && session.selectedNodeId !== target.folder.node_id) {
			session.selectNode(target.folder.node_id, 'REPLACE');
		}
	});

	$effect(() => {
		const next = activeModule ? `Stream Deck: ${activeModule.meta.label}` : 'Stream Deck Editor';
		panelApi.setTitle(next);
	});
</script>

<div class="sd-editor">
	{#if modules.length === 0}
		<div class="sd-empty">
			<h3>No Stream Deck modules</h3>
			<p>Add a <code>Stream Deck</code> module under Modules to start editing.</p>
		</div>
	{:else}
		<header class="sd-strip">
			{#if modules.length > 1}
				<select
					class="sd-module-select"
					value={activeModule?.node_id}
					onchange={(e) =>
						selectModule(Number((e.currentTarget as HTMLSelectElement).value) as NodeId)}>
					{#each modules as module}
						<option value={module.node_id}>{module.meta.label}</option>
					{/each}
				</select>
			{/if}

			<div class="sd-page-tabs" role="tablist" aria-label="Pages">
				{#each pages as page}
					<button
						type="button"
						role="tab"
						aria-selected={activePage?.id === page.id}
						class:selected={activePage?.id === page.id}
						onclick={() => selectPage(page.id)}>{page.label}</button>
				{/each}
				<button type="button" class="sd-page-add" title="New page" onclick={addPage}>+</button>
				{#if activePage && activePage.id !== 'default'}
					<button type="button" class="sd-page-del" title="Delete page" onclick={deleteActivePage}
						>✕</button>
				{/if}
			</div>

			<button
				type="button"
				class="sd-mode"
				class:active={simulateMode}
				aria-pressed={simulateMode}
				onclick={() => (simulateMode = !simulateMode)}>{simulateMode ? 'Simulate' : 'Edit'}</button>
		</header>

		<div class="sd-stage">
			<div
				class="sd-grid"
				style={`grid-template-columns: repeat(${grid[1]}, 1fr); grid-template-rows: repeat(${grid[0]}, 1fr);`}>
				{#each keys as cell (cell.folder.node_id)}
					<div
						class="sd-key"
						class:selected={session?.selectedNodeId === cell.folder.node_id}
						class:pressed={cell.pressed}
						class:sim={simulateMode}
						class:permanent={cell.permanent}
						style={`background-color: ${cell.color};`}
						role="button"
						tabindex="0"
						title={cell.permanent ? 'Un-paged key — edit it on the Default page' : undefined}
						onpointerdown={() => onKeyPointerDown(cell)}
						onpointerup={() => onKeyPointerUp(cell)}
						onpointerleave={() => onKeyPointerUp(cell)}
						ondragover={(e) => e.preventDefault()}
						ondrop={(e) => onDropImage(cell, e)}
						ondblclick={() => {
							if (!simulateMode && !cell.permanent) editingTextKey = cell.folder.node_id;
						}}>
						{#if cell.image}
							<img
								class="sd-key-image"
								src={cell.image}
								alt=""
								draggable="false"
								onerror={(e) => ((e.currentTarget as HTMLImageElement).style.display = 'none')} />
						{/if}
						{#if cell.permanent}
							<span class="sd-key-lock" title="Un-paged (permanent)">🔒</span>
						{/if}
						{#if editingTextKey === cell.folder.node_id}
							<input
								class="sd-key-text-edit"
								value={cell.text}
								use:autofocusInput
								onpointerdown={(e) => e.stopPropagation()}
								onblur={(e) => commitText(cell, (e.currentTarget as HTMLInputElement).value)}
								onkeydown={(e) => {
									if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur();
									if (e.key === 'Escape') editingTextKey = null;
								}} />
						{:else if cell.text}
							<span class="sd-key-text">{cell.text}</span>
						{/if}
						<span class="sd-key-index">{cell.slot + 1}</span>
					</div>
				{/each}
			</div>
			<p class="sd-hint">
				{simulateMode
					? 'Press keys to simulate hardware input.'
					: 'Click a key to edit it in the inspector · double-click to rename · drop an image onto a key.'}
			</p>
		</div>
	{/if}
</div>

<style>
	.sd-editor {
		inline-size: 100%;
		block-size: 100%;
		display: flex;
		flex-direction: column;
		min-block-size: 0;
		overflow: hidden;
	}

	.sd-strip {
		display: flex;
		align-items: center;
		gap: 0.55rem;
		padding: 0.45rem 0.6rem;
		border-bottom: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.35);
	}

	.sd-module-select {
		flex: 0 0 auto;
		background: rgb(from var(--gc-color-background) r g b / 0.42);
		border: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.45);
		border-radius: 0.4rem;
		color: var(--gc-color-text);
		font-size: 0.74rem;
		padding: 0.25rem 0.4rem;
	}

	.sd-page-tabs {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		flex: 1 1 auto;
		min-inline-size: 0;
		overflow-x: auto;
		scrollbar-width: thin;
	}

	.sd-page-tabs button {
		flex: 0 0 auto;
		padding: 0.34rem 0.62rem;
		border-radius: 999rem;
		background: rgb(from var(--gc-color-background) r g b / 0.42);
		border: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.45);
		font-size: 0.72rem;
		line-height: 1;
		color: var(--gc-color-text);
	}

	.sd-page-tabs button.selected {
		background: rgb(from var(--gc-color-selection) r g b / 0.22);
		border-color: rgb(from var(--gc-color-selection) r g b / 0.72);
		color: white;
	}

	.sd-page-add,
	.sd-page-del {
		font-weight: 700;
	}

	.sd-mode {
		flex: 0 0 auto;
		padding: 0.34rem 0.7rem;
		border-radius: 999rem;
		border: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.48);
		background: rgb(from var(--gc-color-background) r g b / 0.42);
		font-size: 0.7rem;
		color: var(--gc-color-text);
	}

	.sd-mode.active {
		background: rgb(from var(--gc-color-selection) r g b / 0.28);
		border-color: rgb(from var(--gc-color-selection) r g b / 0.78);
		color: white;
	}

	.sd-stage {
		flex: 1 1 auto;
		min-block-size: 0;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 0.6rem;
		padding: 0.9rem;
		overflow: auto;
	}

	.sd-grid {
		display: grid;
		gap: 0.5rem;
		inline-size: min(100%, 46rem);
		aspect-ratio: var(--sd-aspect, auto);
	}

	.sd-key {
		position: relative;
		aspect-ratio: 1 / 1;
		border-radius: 0.5rem;
		border: solid 0.08rem rgb(from var(--gc-color-panel-outline) r g b / 0.5);
		display: flex;
		align-items: center;
		justify-content: center;
		overflow: hidden;
		cursor: pointer;
		user-select: none;
		box-shadow: inset 0 0 0 0.06rem rgb(0 0 0 / 0.25);
	}

	.sd-key.sim {
		cursor: pointer;
	}

	.sd-key.selected {
		outline: solid 0.14rem rgb(from var(--gc-color-selection) r g b / 0.95);
		outline-offset: -0.05rem;
	}

	.sd-key.pressed {
		filter: brightness(1.35);
		transform: scale(0.97);
	}

	.sd-key-image {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		object-fit: cover;
		pointer-events: none;
	}

	.sd-key-text {
		position: relative;
		z-index: 1;
		font-size: 0.78rem;
		font-weight: 600;
		color: white;
		text-shadow: 0 0.06rem 0.18rem rgb(0 0 0 / 0.85);
		text-align: center;
		padding: 0.2rem;
		word-break: break-word;
	}

	.sd-key-text-edit {
		position: relative;
		z-index: 2;
		inline-size: 86%;
		background: rgb(0 0 0 / 0.55);
		border: solid 0.06rem rgb(from var(--gc-color-selection) r g b / 0.8);
		border-radius: 0.3rem;
		color: white;
		font-size: 0.76rem;
		text-align: center;
		padding: 0.16rem;
	}

	.sd-key-index {
		position: absolute;
		bottom: 0.12rem;
		right: 0.24rem;
		font-size: 0.55rem;
		opacity: 0.6;
		color: white;
		text-shadow: 0 0.05rem 0.1rem rgb(0 0 0 / 0.9);
		pointer-events: none;
	}

	.sd-hint {
		margin: 0;
		font-size: 0.7rem;
		opacity: 0.7;
		text-align: center;
	}

	.sd-empty {
		margin: auto;
		text-align: center;
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
		padding: 1.2rem;
	}

	.sd-empty h3 {
		margin: 0;
		font-size: 0.85rem;
	}

	.sd-empty p {
		margin: 0;
		font-size: 0.74rem;
		opacity: 0.75;
	}

	.sd-empty :global(code) {
		padding: 0.12rem 0.35rem;
		border-radius: 0.3rem;
		background: rgb(from var(--gc-color-background) r g b / 0.55);
	}
</style>
