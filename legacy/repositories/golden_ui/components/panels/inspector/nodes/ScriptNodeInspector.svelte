<script lang="ts">
	import { appState } from '../../../../store/workbench.svelte';
	import { sendReloadScript, sendSetScriptConfig } from '../../../../store/ui-intents';
	import { hasDesktopHost, openDesktopFileDialog } from '../../../../host/desktop';
	import type { NodeInspectorComponentProps } from '../node-inspector-registry';
	import type { UiNodeDto, UiScriptConfig, UiScriptState } from '../../../../types';
	import SelectNodeButton from '../../../common/SelectNodeButton.svelte';
	import CodeEditor from '../../../common/CodeEditor.svelte';

	let { node, defaultHeader, defaultContent, defaultChildren, level }: NodeInspectorComponentProps =
		$props();

	let session = $derived(appState.session);
	let liveNode: UiNodeDto = $derived(session?.graph.state.nodesById.get(node.node_id) ?? node);
	let hasDesktopDialog = $derived(hasDesktopHost());

	let loading = $state(false);
	let refreshing = $state(false);
	let applying = $state(false);
	let reloading = $state(false);
	let browsing = $state(false);
	let errorMessage = $state('');
	let scriptState = $state<UiScriptState | null>(null);
	let draftConfig = $state<UiScriptConfig | null>(null);
	let inlineCache = $state('');
	let fileCache = $state('');
	let loadVersion = 0;
	let loadedNodeId: number | null = null;
	let loadedSessionClient: unknown = null;

	const cloneConfig = (config: UiScriptConfig): UiScriptConfig => ({
		...config,
		source:
			config.source.kind === 'inline'
				? { kind: 'inline', text: config.source.text }
				: { kind: 'projectFile', path: config.source.path }
	});

	const configsEqual = (left: UiScriptConfig, right: UiScriptConfig): boolean => {
		if (left.source.kind !== right.source.kind) {
			return false;
		}

		if (left.source.kind === 'inline' && right.source.kind === 'inline') {
			return left.source.text === right.source.text;
		}

		if (left.source.kind === 'projectFile' && right.source.kind === 'projectFile') {
			return left.source.path === right.source.path;
		}

		return false;
	};

	let exportNames = $derived.by(() => {
		if (!scriptState) {
			return [] as string[];
		}

		const names = new Set<string>();
		for (const name of scriptState.export_names) {
			if (typeof name === 'string' && name.length > 0) {
				names.add(name);
			}
		}
		for (const entry of scriptState.manifest?.exports ?? []) {
			if (entry.name.length > 0) {
				names.add(entry.name);
			}
		}
		return [...names];
	});

	const toErrorMessage = (error: unknown): string =>
		error instanceof Error ? error.message : 'Unknown error';

	const hasDraftChanges = (): boolean =>
		Boolean(scriptState && draftConfig && !configsEqual(scriptState.config, draftConfig));

	const loadScriptState = async (options: { preserveDraft?: boolean } = {}): Promise<void> => {
		const { preserveDraft = false } = options;
		if (!session || liveNode.node_type !== 'script') {
			return;
		}

		const currentVersion = ++loadVersion;
		const hasEditorState = Boolean(scriptState && draftConfig);
		if (hasEditorState) {
			refreshing = true;
		} else {
			loading = true;
		}
		errorMessage = '';

		try {
			const nextState = await session.client.scriptState(liveNode.node_id);
			if (currentVersion !== loadVersion) {
				return;
			}

			scriptState = nextState;
			if (!preserveDraft || !draftConfig) {
				draftConfig = cloneConfig(nextState.config);
				if (nextState.config.source.kind === 'inline') {
					inlineCache = nextState.config.source.text;
				} else {
					fileCache = nextState.config.source.path;
				}
			}
		} catch (error) {
			if (currentVersion !== loadVersion) {
				return;
			}
			scriptState = null;
			draftConfig = null;
			errorMessage = `Failed to fetch script state: ${toErrorMessage(error)}`;
		} finally {
			if (currentVersion === loadVersion) {
				loading = false;
				refreshing = false;
			}
		}
	};

	$effect(() => {
		const currentSession = session;
		const currentNodeType = liveNode.node_type;
		const currentNodeId = liveNode.node_id;
		const currentClient = currentSession?.client ?? null;

		if (!currentSession || currentNodeType !== 'script') {
			scriptState = null;
			draftConfig = null;
			errorMessage = '';
			loading = false;
			refreshing = false;
			loadedNodeId = null;
			loadedSessionClient = null;
			return;
		}

		if (loadedNodeId === currentNodeId && loadedSessionClient === currentClient) {
			return;
		}

		loadedNodeId = currentNodeId;
		loadedSessionClient = currentClient;
		void loadScriptState();
	});

	const patchDraft = (apply: (config: UiScriptConfig) => UiScriptConfig): void => {
		if (!draftConfig) {
			return;
		}
		draftConfig = apply(draftConfig);
	};

	const setSourceKind = (kind: 'inline' | 'projectFile'): void => {
		if (!draftConfig || draftConfig.source.kind === kind) {
			return;
		}

		if (draftConfig.source.kind === 'inline') {
			inlineCache = draftConfig.source.text;
		} else {
			fileCache = draftConfig.source.path;
		}

		if (kind === 'inline') {
			patchDraft((config) => ({
				...config,
				source: { kind: 'inline', text: inlineCache }
			}));
			void saveConfig(false);
			return;
		}

		patchDraft((config) => ({
			...config,
			source: { kind: 'projectFile', path: fileCache }
		}));
		void saveConfig(false);
	};

	const handleInlineEditorChange = (nextValue: string): void => {
		const value = nextValue;
		inlineCache = value;
		if (draftConfig?.source.kind === 'inline') {
			draftConfig.source.text = value;
		}
	};

	const handleFilePathChange = (value: string): void => {
		fileCache = value;
		if (draftConfig?.source.kind === 'projectFile') {
			draftConfig.source.path = value;
		}
	};

	const handleInlineEditorCommit = async (nextValue: string): Promise<void> => {
		handleInlineEditorChange(nextValue);
		await saveConfig(false);
	};

	const browseFile = async (): Promise<void> => {
		if (
			!draftConfig ||
			draftConfig.source.kind !== 'projectFile' ||
			!hasDesktopDialog ||
			browsing
		) {
			return;
		}

		browsing = true;
		try {
			const selected = await openDesktopFileDialog({
				allowedExtensions: ['js', 'mjs', 'cjs'],
				logTag: 'script-node'
			});
			if (selected) {
				handleFilePathChange(selected);
				void saveConfig(false);
			}
		} finally {
			browsing = false;
		}
	};

	const saveConfig = async (forceReload: boolean): Promise<void> => {
		if (!draftConfig || applying || (!forceReload && !hasDraftChanges())) {
			return;
		}
		const configToApply = cloneConfig(draftConfig);
		applying = true;
		errorMessage = '';
		const ok = await sendSetScriptConfig(liveNode.node_id, configToApply, forceReload);
		applying = false;

		if (!ok) {
			errorMessage = 'Failed to save script configuration.';
			return;
		}

		if (scriptState) {
			scriptState = {
				...scriptState,
				config: cloneConfig(configToApply)
			};
		}

		if (forceReload) {
			await loadScriptState({ preserveDraft: true });
		}
	};

	const reloadRuntime = async (): Promise<void> => {
		if (reloading) {
			return;
		}
		reloading = true;
		errorMessage = '';
		const ok = await sendReloadScript(liveNode.node_id);
		reloading = false;
		if (!ok) {
			errorMessage = 'Failed to request script runtime reload.';
			return;
		}
		await loadScriptState({ preserveDraft: true });
	};
</script>

{#snippet scriptHeaderExtra()}
	{#if level > 0}
		<SelectNodeButton {node} />
	{/if}
{/snippet}

{#snippet scriptContent()}
	<div class="script-node-inspector-body">
		<div class="toolbar">
			<button
				type="button"
				class="ghost"
				disabled={!draftConfig || applying || reloading}
				onclick={() => {
					void reloadRuntime();
				}}>
				{reloading ? 'Reloading...' : 'Reload Runtime'}
			</button>
			<div class="spacer"></div>
			<select
				value={draftConfig?.source.kind}
				onchange={(event) => {
					setSourceKind((event.target as HTMLSelectElement).value as 'inline' | 'projectFile');
				}}>
				<option value="inline">Inline</option>
				<option value="projectFile">Project File</option>
			</select>
			<div class="status subtle save-hint">Auto-save on blur or Ctrl+S</div>
		</div>

		{#if !draftConfig || !scriptState}
			{#if loading}
				<div class="status">Loading script state...</div>
			{:else}
				<div class="status error">
					{errorMessage || 'Script state unavailable for this node.'}
				</div>
			{/if}
		{:else}
			<div class="script-main" class:with-inline-editor={draftConfig.source.kind === 'inline'}>
				{#if draftConfig.source.kind === 'inline'}
					{#if level == 0}
						<div class="source-editor">
							<CodeEditor
								value={draftConfig.source.text}
								language="javascript"
								minHeight="0rem"
								fill={true}
								persistKey={`script-inline-${liveNode.node_id}`}
								commitOnBlur={true}
								oncommit={handleInlineEditorCommit} />
						</div>
					{/if}
				{:else}
					<div class="file-source-row">
						<input
							type="text"
							value={draftConfig.source.path}
							placeholder="Select script file"
							onchange={(event) => {
								const value = (event.target as HTMLInputElement).value;
								handleFilePathChange(value);
								void saveConfig(false);
							}} />
						<button
							type="button"
							disabled={!hasDesktopDialog || browsing}
							onclick={() => {
								void browseFile();
							}}>
							{browsing ? '...' : 'Browse'}
						</button>
					</div>
					{#if !hasDesktopDialog}
						<div class="status subtle">
							Desktop file picker unavailable. Enter the path manually.
						</div>
					{/if}
				{/if}

				<div class="script-footer">
					<div class="runtime-summary">
						<div><strong>Runtime:</strong> QuickJS</div>
						<div>
							<strong>Effective Update Rate:</strong>
							{scriptState.effective_update_rate_hz
								? `${scriptState.effective_update_rate_hz} Hz`
								: 'passive'}
						</div>
						<div>
							<strong>Script API:</strong>
							{scriptState.manifest ? scriptState.manifest.api_version : 'not loaded'}
						</div>
					</div>

					{#if exportNames.length > 0}
						<div class="exports">
							<strong>Exports</strong>
							<div class="export-list">
								{#each exportNames as name}
									<span class="export-chip">{name}</span>
								{/each}
							</div>
						</div>
					{/if}

					{#if errorMessage}
						<div class="status error">{errorMessage}</div>
					{/if}
					{#if refreshing}
						<div class="status subtle">Refreshing script state...</div>
					{/if}
				</div>
			</div>
		{/if}
	</div>

	{@render defaultChildren?.()}
{/snippet}

{#if liveNode.node_type === 'script'}
	{@render defaultHeader?.(scriptHeaderExtra)}
	{@render defaultContent?.(scriptContent, 'script-node-inspector')}
{/if}

<style>
	:global(.script-node-inspector) {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
		height: 100%;
		min-height: 0;
		padding: 0.35rem;
		border-radius: 0.45rem;
		border: solid 0.06rem rgba(255, 255, 255, 0.08);
		background: rgba(255, 255, 255, 0.02);
		font-size: 0.72rem;
		overflow: hidden;
	}

	.script-main {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
	}

	.script-main.with-inline-editor {
		flex: 1 1 auto;
		min-height: 0;
		overflow: hidden;
	}

	.toolbar {
		display: flex;
		align-items: center;
		gap: 0.3rem;
		flex-wrap: wrap;
	}

	.save-hint {
		margin-left: auto;
		font-size: 0.62rem;
	}

	.toolbar button {
		height: 1.5rem;
		padding: 0 0.55rem;
		border-radius: 0.3rem;
		font-size: 0.68rem;
		border: solid 0.06rem rgba(255, 255, 255, 0.14);
		background: rgba(255, 255, 255, 0.06);
		color: var(--text-color);
	}

	.toolbar button:disabled {
		opacity: 0.5;
	}

	input,
	select {
		border: solid 0.06rem rgba(255, 255, 255, 0.16);
		border-radius: 0.25rem;
		color: var(--text-color);
		font-size: 0.7rem;
		padding: 0.25rem 0.35rem;
		box-sizing: border-box;
	}

	input {
		background-color: var(--bg-color);
		line-height: 1.3;
	}

	.source-editor {
		flex: 1 1 auto;
		min-height: 0;
		overflow: hidden;
	}

	.file-source-row {
		display: flex;
		align-items: center;
		gap: 0.3rem;
	}

	.file-source-row input {
		flex: 1 1 auto;
		min-width: 0;
	}

	.file-source-row button {
		height: 1.5rem;
		padding: 0 0.5rem;
		border-radius: 0.3rem;
		font-size: 0.68rem;
		border: solid 0.06rem rgba(255, 255, 255, 0.14);
		background: rgba(255, 255, 255, 0.06);
		color: var(--text-color);
	}

	.runtime-summary {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 0.25rem;
		font-size: 0.66rem;
		opacity: 0.88;
	}

	.script-footer {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}

	.exports {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
	}

	.export-list {
		display: flex;
		flex-wrap: wrap;
		gap: 0.25rem;
	}

	.export-chip {
		display: inline-flex;
		align-items: center;
		height: 1.25rem;
		padding: 0 0.4rem;
		border-radius: 999rem;
		font-size: 0.62rem;
		border: solid 0.06rem rgba(255, 255, 255, 0.14);
		background: rgba(255, 255, 255, 0.04);
	}

	.status {
		font-size: 0.66rem;
		opacity: 0.86;
	}

	.status.subtle {
		opacity: 0.65;
	}

	.status.error {
		color: var(--gc-color-warning);
		opacity: 1;
	}

	@media (max-width: 50rem) {
		.runtime-summary {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
