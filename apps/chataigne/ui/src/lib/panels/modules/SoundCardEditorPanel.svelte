<script lang="ts">
	import { appState } from 'golden_ui/store/workbench.svelte';
	import {
		NodeInspector,
		type NodeId,
		type PanelProps,
		type PanelState,
		type UiNodeDto
	} from 'golden_ui';
	import { resolveModuleEditor } from './module-editor-registry';

	type EditorParams = {
		moduleNodeId?: NodeId;
	};

	const SOUND_CARD_MODULE_TYPE = 'sound_card_module';
	const SECTION_KEYS = ['connection', 'parameters', 'values'] as const;

	let props: PanelProps = $props();
	let updatedPanelState = $state<PanelState | null>(null);
	let panelState = $derived(
		updatedPanelState ?? {
			panelId: props.panelId,
			panelType: props.panelType,
			title: props.title,
			params: props.params
		}
	);

	export const setPanelState = (next: PanelState): void => {
		updatedPanelState = next;
	};

	let session = $derived(appState.session);
	let nodes = $derived(session?.graph.state.nodesById ?? new Map<NodeId, UiNodeDto>());
	let panelParams = $derived((panelState.params ?? {}) as EditorParams);
	let soundCardModules = $derived(
		[...nodes.values()].filter((node) => node.node_type === SOUND_CARD_MODULE_TYPE)
	);
	let activeModule = $derived.by(() => {
		const requested = panelParams.moduleNodeId;
		if (requested !== undefined) {
			const candidate = nodes.get(requested);
			if (candidate?.node_type === SOUND_CARD_MODULE_TYPE) return candidate;
		}
		return soundCardModules[0] ?? null;
	});

	const declaredKey = (node: UiNodeDto): string => node.decl_id.split('/').at(-1) ?? node.decl_id;

	const childByKey = (parent: UiNodeDto, key: string): UiNodeDto | null =>
		parent.children
			.map((childId) => nodes.get(childId))
			.find(
				(child): child is UiNodeDto =>
					child !== undefined &&
					(child.decl_id === key || declaredKey(child) === key || child.meta.short_name === key)
			) ?? null;

	let sections = $derived.by(() =>
		activeModule
			? SECTION_KEYS.map((key) => childByKey(activeModule, key)).filter(
					(section): section is UiNodeDto => section !== null
				)
			: []
	);

	const panelTitle = (module: UiNodeDto): string =>
		resolveModuleEditor(module)?.title(module) ?? `Sound Card: ${module.meta.label}`;

	$effect(() => {
		if (!activeModule) return;
		const title = panelTitle(activeModule);
		if (panelState.title !== title) props.panelApi.setTitle(title);
	});

	const selectModule = (event: Event): void => {
		const nodeId = Number((event.currentTarget as HTMLSelectElement).value);
		const module = nodes.get(nodeId);
		if (!module || module.node_type !== SOUND_CARD_MODULE_TYPE) return;
		const params = { ...panelState.params, moduleNodeId: module.node_id };
		const next = { ...panelState, title: panelTitle(module), params };
		updatedPanelState = next;
		props.panelApi.updateParams(params);
		props.panelApi.setTitle(next.title);
	};
</script>

<div class="sound-card-editor">
	<header class="editor-header">
		<div>
			<h1>Sound Card</h1>
			<p>Connection, routing, channel levels, and processing.</p>
		</div>
		<label>
			<span>Module</span>
			<select value={String(activeModule?.node_id ?? '')} onchange={selectModule}>
				{#if soundCardModules.length === 0}
					<option value="">No Sound Card module</option>
				{/if}
				{#each soundCardModules as module (module.node_id)}
					<option value={String(module.node_id)}>{module.meta.label}</option>
				{/each}
			</select>
		</label>
	</header>

	{#if activeModule}
		<main>
			{#each sections as section, index (section.node_id)}
				<NodeInspector
					nodes={[section]}
					level={0}
					order={sections.length === 1
						? 'solo'
						: index === 0
							? 'first'
							: index === sections.length - 1
								? 'last'
								: ''} />
			{/each}
		</main>
	{:else}
		<p class="missing">No Sound Card module found.</p>
	{/if}
</div>

<style>
	.sound-card-editor {
		display: flex;
		flex-direction: column;
		block-size: 100%;
		min-block-size: 0;
		background: var(--gc-color-bg);
		color: var(--gc-color-text);
	}

	.editor-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.8rem 1rem;
		border-block-end: 0.0625rem solid var(--gc-color-border);
		background: var(--gc-color-bg-light);
	}

	h1,
	p {
		margin: 0;
	}

	h1 {
		font-size: 1.05rem;
	}

	.editor-header p {
		margin-block-start: 0.18rem;
		color: var(--gc-color-text-muted);
		font-size: 0.72rem;
	}

	.editor-header label {
		display: grid;
		gap: 0.2rem;
		min-inline-size: min(15rem, 42%);
		color: var(--gc-color-text-muted);
		font-size: 0.7rem;
	}

	select {
		min-block-size: 2rem;
		padding-inline: 0.45rem;
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.35rem;
		background: var(--gc-color-bg-lighter);
		color: var(--gc-color-text);
		font: inherit;
	}

	select:focus-visible {
		outline: 0.15rem solid var(--gc-color-accent);
		outline-offset: 0.1rem;
	}

	main {
		display: grid;
		align-content: start;
		gap: 0.75rem;
		min-block-size: 0;
		padding: 0.9rem;
		overflow: auto;
	}

	.missing {
		margin: auto;
		padding: 1rem;
		color: var(--gc-color-text-muted);
		font-size: 0.8rem;
	}

	@media (max-width: 42rem) {
		.editor-header {
			align-items: stretch;
			flex-direction: column;
		}

		.editor-header label {
			min-inline-size: 0;
		}
	}
</style>
