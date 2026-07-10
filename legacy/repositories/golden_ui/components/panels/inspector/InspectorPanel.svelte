<script lang="ts">
	import { untrack, type Snippet } from 'svelte';
	import NodeDataInspector from './NodeDataInspector.svelte';
	import NodeInspector from './NodeInspector.svelte';
	import { resolveNodeInspector } from './node-inspector-registry';

	import { appState } from '../../../store/workbench.svelte.ts';
	import {
		readPanelPersistedState,
		writePanelPersistedState
	} from '../../../dockview/panel-persistence';
	import type { PanelProps, PanelState } from '../../../dockview/panel-types.ts';
	import type { NodeId } from '../../../types';
	import { slide } from 'svelte/transition';
	import { getContainerColorForNode, getIconURLForNode } from '../../../store/node-types';
	import { sendPatchMetaIntent } from '../../../store/ui-intents';
	import EnableButton from '../../common/EnableButton.svelte';
	import Watcher from '../../common/Watcher.svelte';
	import type { WatcherUiSettings } from '../../common/watcher/watcher-utils';
	import NodeAddButton from '../../common/NodeAddButton.svelte';
	import Arrow from '../../common/Arrow.svelte';

	let { panelApi, panelId, panelType, title, params }: PanelProps = $props();

	let panel = $state<PanelState>({
		panelId: '',
		panelType: '',
		title: '',
		params: {}
	});

	export const setPanelState = (next: PanelState): void => {
		panel = next;
		applyPersistedState(next.params);
	};

	let session = $derived(appState.session);
	let graphState = $derived(session?.graph.state ?? null);
	let selectedNodes = $derived(session?.getSelectedNodes() ?? []);
	let selectedNode = $derived(selectedNodes.length === 1 ? selectedNodes[0] : null);

	let inspectorLocked = $state(false);
	let lockedNodeId = $state<NodeId | null>(null);
	let inspectedHistory = $state<NodeId[]>([]);
	let inspectedHistoryIndex = $state(-1);
	const INSPECT_HISTORY_LIMIT = 128;

	let inspectedNodeId = $derived.by((): NodeId | null => {
		if (inspectorLocked) {
			return lockedNodeId;
		}
		return selectedNode?.node_id ?? null;
	});
	let node = $derived.by(() => {
		if (inspectedNodeId === null || !graphState) {
			return null;
		}
		return graphState.nodesById.get(inspectedNodeId) ?? null;
	});
	let inspectedNodes = $derived(node ? [node] : []);

	let iconURL = $derived(node ? getIconURLForNode(node) : null);
	let headerAccentColor = $derived(
		node ? getContainerColorForNode(node) : 'rgba(124, 138, 162, 1)'
	);
	let isNameChangeable = $derived(node?.meta.user_permissions.can_edit_name ?? false);
	let warnings = $derived(node ? session?.getNodeVisibleWarnings(node.node_id) : null);
	let parentNodeId = $derived.by((): NodeId | null => {
		if (!node || !graphState) {
			return null;
		}
		return graphState.parentById.get(node.node_id) ?? null;
	});

	let canGoHistoryBackward = $derived(inspectedHistoryIndex > 0);
	let canGoHistoryForward = $derived(
		inspectedHistoryIndex >= 0 && inspectedHistoryIndex < inspectedHistory.length - 1
	);
	let canInspectParent = $derived(parentNodeId !== null);
	let computedPanelTitle = $derived(node ? `Inspector: ${node.meta.label}` : 'Inspector');

	let warningCount = $derived(warnings ? warnings.length : 0);
	let customInspectorEntry = $derived(node ? resolveNodeInspector(node) : null);
	let CustomPanelHeaderComponent = $derived(customInspectorEntry?.panelHeaderComponent ?? null);
	let renameInputElem: HTMLInputElement | null = $state(null as HTMLInputElement | null);
	let renamingState = $state({
		isRenaming: false,
		renameDraft: ''
	});

	let dataInspectorCollapsed = $state(true);
	let watcherSettingsByParam = $state<Record<string, Partial<WatcherUiSettings>>>({});
	let badgeOver = $state(false);
	const footerHoverToken = Symbol('inspector-panel-footer-hover');
	let headerHoverActive = $state(false);

	const WATCHER_SETTINGS_CACHE_LIMIT = 256;

	const isRecord = (value: unknown): value is Record<string, unknown> =>
		typeof value === 'object' && value !== null && !Array.isArray(value);

	const sanitizeWatcherSettingsMap = (
		value: unknown
	): Record<string, Partial<WatcherUiSettings>> => {
		if (!isRecord(value)) {
			return {};
		}

		const entries: Array<[string, Partial<WatcherUiSettings>]> = [];
		for (const [paramKey, rawSettings] of Object.entries(value)) {
			if (!isRecord(rawSettings)) {
				continue;
			}
			entries.push([paramKey, rawSettings as Partial<WatcherUiSettings>]);
		}
		return Object.fromEntries(entries);
	};

	const sanitizeNodeId = (value: unknown): NodeId | null => {
		const parsed = Number(value);
		if (!Number.isFinite(parsed)) {
			return null;
		}
		return Math.floor(parsed) as NodeId;
	};

	interface InspectorPersistedState {
		dataInspectorCollapsed?: boolean;
		watcherSettingsByParam?: Record<string, Partial<WatcherUiSettings>>;
		inspectorLocked?: boolean;
		inspectorLockedNodeId?: NodeId | null;
	}

	const applyPersistedState = (nextParams: PanelState['params']): void => {
		const persistedState = readPanelPersistedState<InspectorPersistedState>(nextParams);

		if (
			typeof persistedState.dataInspectorCollapsed === 'boolean' &&
			persistedState.dataInspectorCollapsed !== dataInspectorCollapsed
		) {
			dataInspectorCollapsed = persistedState.dataInspectorCollapsed;
		}

		watcherSettingsByParam = sanitizeWatcherSettingsMap(persistedState.watcherSettingsByParam);

		if (
			typeof persistedState.inspectorLocked === 'boolean' ||
			persistedState.inspectorLockedNodeId !== undefined
		) {
			const nextLockedNodeId = sanitizeNodeId(persistedState.inspectorLockedNodeId);
			const nextLocked = Boolean(persistedState.inspectorLocked) && nextLockedNodeId !== null;
			inspectorLocked = nextLocked;
			lockedNodeId = nextLocked ? nextLockedNodeId : null;
		}
	};

	const persistState = (nextState: Partial<InspectorPersistedState>): void => {
		writePanelPersistedState(panelApi, nextState);
	};

	const setInspectorLock = (nextLocked: boolean, targetNodeId: NodeId | null): void => {
		const normalizedLocked = nextLocked && targetNodeId !== null;
		const normalizedNodeId = normalizedLocked ? targetNodeId : null;
		if (inspectorLocked === normalizedLocked && lockedNodeId === normalizedNodeId) {
			return;
		}

		inspectorLocked = normalizedLocked;
		lockedNodeId = normalizedNodeId;
		persistState({
			inspectorLocked: normalizedLocked,
			inspectorLockedNodeId: normalizedNodeId
		});
	};

	const lockToNode = (targetNodeId: NodeId | null): void => {
		if (targetNodeId === null) {
			return;
		}
		setInspectorLock(true, targetNodeId);
	};

	const goToHistoryIndex = (targetIndex: number): void => {
		if (targetIndex < 0 || targetIndex >= inspectedHistory.length) {
			return;
		}

		const targetNodeId = inspectedHistory[targetIndex] ?? null;
		if (targetNodeId === null) {
			return;
		}

		inspectedHistoryIndex = targetIndex;
		lockToNode(targetNodeId);
	};

	const inspectPreviousNode = (): void => {
		goToHistoryIndex(inspectedHistoryIndex - 1);
	};

	const inspectNextNode = (): void => {
		goToHistoryIndex(inspectedHistoryIndex + 1);
	};

	const inspectParentNode = (): void => {
		lockToNode(parentNodeId);
	};

	const resetToSelectedNode = (): void => {
		setInspectorLock(false, null);
	};

	const toggleInspectorLock = (): void => {
		if (inspectorLocked) {
			setInspectorLock(false, null);
			return;
		}
		lockToNode(node?.node_id ?? null);
	};

	const toggleDataInspector = (): void => {
		dataInspectorCollapsed = !dataInspectorCollapsed;
		persistState({ dataInspectorCollapsed });
	};

	let selectedWatcherParamKey = $derived(node && node.data.kind === 'parameter' ? node.uuid : null);
	let selectedWatcherSettings = $derived(
		selectedWatcherParamKey ? (watcherSettingsByParam[selectedWatcherParamKey] ?? {}) : {}
	);

	const persistWatcherSettingsForSelectedParam = (nextPatch: Partial<WatcherUiSettings>): void => {
		if (!selectedWatcherParamKey) {
			return;
		}

		const currentSettings = watcherSettingsByParam[selectedWatcherParamKey] ?? {};
		const mergedSettings = {
			...currentSettings,
			...nextPatch
		};

		let didChange = false;
		for (const [key, value] of Object.entries(nextPatch)) {
			if ((currentSettings as Record<string, unknown>)[key] !== value) {
				didChange = true;
				break;
			}
		}
		if (!didChange) {
			return;
		}

		let nextMap: Record<string, Partial<WatcherUiSettings>> = {
			...watcherSettingsByParam,
			[selectedWatcherParamKey]: mergedSettings
		};

		const keys = Object.keys(nextMap);
		if (keys.length > WATCHER_SETTINGS_CACHE_LIMIT) {
			nextMap = Object.fromEntries(
				Object.entries(nextMap).slice(keys.length - WATCHER_SETTINGS_CACHE_LIMIT)
			);
		}

		watcherSettingsByParam = nextMap;
		persistState({ watcherSettingsByParam: nextMap });
	};

	$effect(() => {
		if (!renamingState.isRenaming || !renameInputElem) {
			return;
		}

		renameInputElem.focus();
		renameInputElem.select();
	});

	$effect(() => {
		const currentNodeId = node?.node_id;
		if (currentNodeId === undefined) {
			return;
		}

		const currentHistoryEntry =
			inspectedHistoryIndex >= 0 ? inspectedHistory[inspectedHistoryIndex] : null;
		if (currentHistoryEntry === currentNodeId) {
			return;
		}

		const baseHistory =
			inspectedHistoryIndex >= 0 ? inspectedHistory.slice(0, inspectedHistoryIndex + 1) : [];
		baseHistory.push(currentNodeId);

		const trimmedHistory =
			baseHistory.length > INSPECT_HISTORY_LIMIT
				? baseHistory.slice(baseHistory.length - INSPECT_HISTORY_LIMIT)
				: baseHistory;
		inspectedHistory = trimmedHistory;
		inspectedHistoryIndex = trimmedHistory.length - 1;
	});

	$effect(() => {
		if (!graphState || inspectedHistory.length === 0) {
			return;
		}

		const validHistory = inspectedHistory.filter((entry) => graphState.nodesById.has(entry));
		if (validHistory.length === inspectedHistory.length) {
			return;
		}

		const currentEntry = inspectedHistory[inspectedHistoryIndex] ?? null;
		inspectedHistory = validHistory;

		if (validHistory.length === 0) {
			inspectedHistoryIndex = -1;
			return;
		}

		if (currentEntry !== null) {
			const preservedIndex = validHistory.lastIndexOf(currentEntry);
			if (preservedIndex >= 0) {
				inspectedHistoryIndex = preservedIndex;
				return;
			}
		}

		inspectedHistoryIndex = Math.min(inspectedHistoryIndex, validHistory.length - 1);
	});

	$effect(() => {
		if (!inspectorLocked || lockedNodeId === null || !graphState) {
			return;
		}
		if (graphState.nodesById.has(lockedNodeId)) {
			return;
		}
		setInspectorLock(false, null);
	});

	let lastAppliedPanelTitle = $state('');
	$effect(() => {
		if (computedPanelTitle === lastAppliedPanelTitle) {
			return;
		}
		lastAppliedPanelTitle = computedPanelTitle;
		panelApi.setTitle(computedPanelTitle);
	});

	const startRename = (): void => {
		if (!node || !isNameChangeable) {
			return;
		}
		renamingState.renameDraft = node.meta.label;
		renamingState.isRenaming = true;
	};

	const cancelRename = (): void => {
		if (!node) {
			renamingState.isRenaming = false;
			return;
		}
		renamingState.renameDraft = node.meta.label;
		renamingState.isRenaming = false;
	};

	const commitRename = async (): Promise<void> => {
		if (!node || !isNameChangeable) {
			renamingState.isRenaming = false;
			return;
		}

		const nextName = String(renamingState.renameDraft ?? '').trim();
		if (!nextName || nextName === node.meta.label) {
			renamingState.isRenaming = false;
			return;
		}

		await sendPatchMetaIntent(node.node_id, { label: nextName });
		renamingState.isRenaming = false;
	};

	$effect(() => {
		panel = {
			panelId,
			panelType,
			title,
			params
		};
		applyPersistedState(params);
	});

	const handleHeaderPointerEnter = (): void => {
		headerHoverActive = true;
	};

	const handleHeaderPointerLeave = (): void => {
		headerHoverActive = false;
	};

	$effect(() => {
		if (!headerHoverActive || !node) {
			untrack(() => {
				session?.clearFooterHover(footerHoverToken);
			});
			return;
		}
		untrack(() => {
			session?.setFooterHover(footerHoverToken, node.node_id);
		});
		return () => {
			untrack(() => {
				session?.clearFooterHover(footerHoverToken);
			});
		};
	});
</script>

<div class="inspector">
	{#snippet builtInHeader(headerExtra?: Snippet)}
		<div
			class="inspector-header"
			role="group"
			data-node-id={node?.node_id}
			style="--node-accent-color: {headerAccentColor}"
			onpointerenter={handleHeaderPointerEnter}
			onpointerleave={handleHeaderPointerLeave}>
			{#if node == null}
				<span class="title-text">No node selected</span>
			{:else}
				<span class="header-icon">
					{#if iconURL}
						<img src={iconURL} alt="" />
					{/if}
				</span>

				{#if node.meta.can_be_disabled}
					<EnableButton {node} />
				{/if}

				{#if renamingState.isRenaming}
					<input
						class="title-input"
						bind:this={renameInputElem}
						bind:value={renamingState.renameDraft}
						onblur={() => {
							void commitRename();
						}}
						onkeydown={(e) => {
							if (e.key === 'Enter') {
								void commitRename();
								return;
							}
							if (e.key === 'Escape') {
								cancelRename();
							}
						}} />
				{:else}
					<span
						class="title-text {isNameChangeable ? 'name-changeable' : ''}"
						role="textbox"
						tabindex="-1"
						ondblclick={() => {
							startRename();
						}}
						title={isNameChangeable ? 'Double-click to rename' : undefined}>
						{node.meta.label}
					</span>
				{/if}

				{#if headerExtra}
					{@render headerExtra()}
					<!-- {:else} -->
				{/if}
				<div class="spacer"></div>

				<!-- <div
					class="node-id-badge"
					role="button"
					tabindex="-1"
					onmouseenter={() => (badgeOver = true)}
					onmouseleave={() => (badgeOver = false)}>
					<button
						class="copy-button"
						title="Copy path to clipboard"
						onclick={() => {
							if (node) {
								navigator.clipboard.writeText(node.uuid);
							}
						}}>
						Copy
					</button>
					{#if badgeOver}
						<div class="node-id" transition:slide={{ duration: 200, axis: 'x' }}>{node.decl_id}</div>
					{/if}
				</div> -->
			{/if}

			{#if node}
				<NodeAddButton {node} />
			{/if}

			<div class="inspector-nav-tools">
				<button
					type="button"
					class="inspector-nav-button"
					title="Inspect parent node"
					disabled={!canInspectParent}
					onclick={inspectParentNode}>
					<Arrow direction="up" />
				</button>
				<button
					type="button"
					class="inspector-nav-button"
					title="Inspect previous node"
					disabled={!canGoHistoryBackward}
					onclick={inspectPreviousNode}>
					<Arrow direction="left" />
				</button>
				<button
					type="button"
					class="inspector-nav-button"
					title="Inspect next node"
					disabled={!canGoHistoryForward}
					onclick={inspectNextNode}>
					<Arrow direction="right" />
				</button>

				<button
					type="button"
					class="inspector-nav-button lock-toggle"
					title={inspectorLocked ? 'Unlock inspector from node' : 'Lock inspector to current node'}
					disabled={node === null}
					class:is-active={inspectorLocked}
					onclick={toggleInspectorLock}>
				</button>
			</div>
		</div>
	{/snippet}

	{#if node !== null && CustomPanelHeaderComponent}
		<CustomPanelHeaderComponent {node}>
			{#snippet defaultHeader(headerExtra?: Snippet)}
				{@render builtInHeader(headerExtra)}
			{/snippet}
		</CustomPanelHeaderComponent>
	{:else}
		{@render builtInHeader()}
	{/if}

	{#if node !== null}
		{#if warningCount > 0}
			<div class="warning-info" transition:slide={{ duration: 200 }}>
				{#each warnings as warning}
					<div class="warning-item" transition:slide={{ duration: 200 }}>
						<div class="warning-item-message">
							{#if warning.sourceNodeId != node.node_id}
								<strong>{warning.sourceNodeLabel}:</strong>
							{/if}
							<span>{warning.message}</span>
						</div>
						{#if warning.detail?.trim()}
							<pre class="warning-detail">{warning.detail.trim()}</pre>
						{/if}
					</div>
				{/each}
			</div>
		{/if}

		<div class="inspector-content">
			{#if node.data.kind === 'parameter'}
				<div class="watcher-wrapper">
					<Watcher
						{node}
						persistedSettings={selectedWatcherSettings}
						onSettingsChange={persistWatcherSettingsForSelectedParam} />
				</div>
			{/if}
			<NodeInspector nodes={inspectedNodes} level={0} />
		</div>

		<div class="data-inspector {dataInspectorCollapsed ? 'collapsed' : ''}">
			<div
				class="data-inspector-header"
				role="button"
				tabindex="0"
				onclick={toggleDataInspector}
				onkeydown={(e) => {
					if (e.key !== 'Enter' && e.key !== ' ') return;
					e.preventDefault();
					toggleDataInspector();
				}}>
				<span>Raw Data</span>
				<div class="arrow {dataInspectorCollapsed ? 'up' : 'down'}"></div>
			</div>
			{#if !dataInspectorCollapsed}
				<div class="data-inspector-content" transition:slide|local={{ duration: 200 }}>
					<NodeDataInspector nodes={inspectedNodes} />
				</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	.inspector {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.inspector-header {
		border-bottom: solid 1px rgba(from var(--node-accent-color) r g b / 0.24);
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.25rem 0;
	}

	.inspector-header .header-icon {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 1.7rem;
		height: 1.7rem;
		border-radius: 0.4rem;
		background: rgba(from var(--node-accent-color) r g b / 0.12);
		box-shadow: inset 0 0 0 1px rgba(from var(--node-accent-color) r g b / 0.16);
	}

	.inspector-header .header-icon img {
		width: 1.2rem;
		height: 1.2rem;
		vertical-align: middle;
		filter: drop-shadow(0 0 0.25rem rgba(from var(--node-accent-color) r g b / 0.28));
	}

	.inspector-header .title-text {
		font-weight: 600;
	}

	.inspector-header .title-text.name-changeable {
		cursor: text;
	}

	.inspector-header .title-input {
		height: 1.5rem;
		min-width: 10rem;
		background-color: var(--bg-color);
		color: var(--text-color);
		font-size: 0.8rem;
		outline: 1px solid rgba(255, 255, 255, 0.2);
		border-radius: 0.25rem;
		padding: 0 0.35rem;
	}

	.inspector-nav-tools {
		display: inline-flex;
		align-items: center;
	}

	.inspector-nav-button {
		border-radius: 0.25rem;
		font-size: 0.65rem;
		opacity: 0.6;
		height: 1rem;
		width: 1rem;
		padding: 0;
		margin: 0 0.2rem;
	}

	.inspector-nav-button:hover:not(:disabled) {
		opacity: 1;
	}

	.inspector-nav-button:disabled {
		opacity: 0.2;
		cursor: default;
	}

	.lock-toggle {
		background-image: url('../../../style/icons/lock.svg');
		background-repeat: no-repeat;
		background-position: center;
		background-size: contain;
		opacity: 0.8;
		filter: grayscale(100%);
		transition:
			opacity 0.2s ease,
			filter 0.2s ease;
	}

	.lock-toggle.is-active {
		opacity: 1;
		filter: drop-shadow(0 0 3px rgba(255, 255, 0, 0.6));
	}

	.warning-info {
		display: flex;
		flex-direction: column;
		padding: 0.5rem 0.25rem;
		font-size: 0.8rem;
		color: var(--gc-color-warning);
		background-color: rgb(from var(--gc-color-warning) r g b / 20%);
		background-image: url('../../../style/icons/warning.svg');
		background-position: top 0.2rem right 0.3rem;
		background-repeat: no-repeat;
		background-size: 1rem;
		margin: 0.25rem 0;
		border-radius: 0.25rem;
		gap: 0.15rem;
	}

	.warning-item {
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
		padding-inline-end: 1.4rem;
	}

	.warning-item-message {
		display: flex;
		flex-wrap: wrap;
		gap: 0.25rem;
		align-items: baseline;
	}

	.warning-detail {
		margin: 0;
		white-space: pre-wrap;
		font: inherit;
		font-size: 0.72rem;
		line-height: 1.35;
		color: color-mix(in srgb, var(--gc-color-warning) 78%, var(--gc-color-text) 22%);
		user-select: text;
	}

	.inspector-content {
		flex: 1 1 auto;
		min-height: 0;
		overflow-x: hidden;
		overflow: -moz-scrollbars-vertical;
		overflow-y: auto;
		scrollbar-gutter: stable;
		padding-right: 0.3rem;
	}

	.watcher-wrapper {
		padding: 0.35rem 0.15rem 0.2rem;
	}

	.data-inspector {
		max-height: 50%;
		display: flex;
		flex-direction: column;
		align-items: center;
	}

	.data-inspector .data-inspector-header {
		display: flex;
		padding: 0.3rem 0.5rem;
		border-radius: 0.3rem 0.3rem 0 0;
		justify-content: center;
		font-size: 0.7rem;
		background-color: rgba(255, 255, 255, 0.05);
		border-bottom: none;
		font-size: 0.7rem;
		cursor: pointer;
		transition: background-color 0.2s ease;
		border-top: solid 1px rgba(255, 255, 255, 0.1);
	}

	.data-inspector.collapsed .data-inspector-header {
		border-radius: 0.3rem;
	}

	.data-inspector .data-inspector-header:hover {
		background-color: rgba(255, 255, 255, 0.1);
	}

	.arrow {
		margin-left: 0.5rem;
	}

	.data-inspector-content {
		overflow-y: auto;
		scrollbar-gutter: stable;
		background-color: rgba(0, 0, 0, 0.6);
		border-top: solid 2px rgba(255, 255, 255, 0.2);
		padding: 0.5rem;
		box-sizing: border-box;
		border-radius: 0.5rem;
		font-size: 0.7rem;
		color: var(--text-color);
		width: 100%;
		height: 100%;
	}
</style>
