<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { UiCreatableUserItem, UiEditIntent, UiNodeDto } from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import type {
		FormulaPreviewModeDto,
		ManagedItemDto,
		ProcessorUiDto,
		StateMachinePreviewCatalogDto,
		StateMachineRuntimePreviewDto
	} from '../../state_machine/generated';
	import { contextKeyId } from '../preview/formulaPreviewSessionStore.svelte';
	import {
		STATE_MACHINE_RUNTIME_PREVIEW_CATALOG_TOPIC,
		STATE_MACHINE_RUNTIME_PREVIEW_DEMAND_TOPIC,
		STATE_MACHINE_RUNTIME_PREVIEW_TOPIC
	} from '../preview/formulaOutputPreviewStore.svelte';
	import {
		directChild,
		hasTupleManagedRegions,
		mappingCreateIntent,
		mappingDuplicateIntent,
		mappingMoveIntent,
		mappingPreviewDemand,
		mappingPreviewMode,
		mappingRegionNode
	} from '../mappingInspectorModel';
	import MappingRegionList from './MappingRegionList.svelte';

	let { node, fallback }: { node: UiNodeDto; fallback?: Snippet } = $props();
	const subscriptionId = `mapping-inspector:${crypto.randomUUID()}`;
	let session = $derived(appState.session);
	let graph = $derived(session?.graph.state ?? null);
	let liveNode = $derived(graph?.nodesById.get(node.node_id) ?? node);
	let manager = $derived.by((): UiNodeDto | null => {
		if (!graph) return null;
		let current: UiNodeDto | undefined = liveNode;
		while (current) {
			if (current.node_type === 'state_machine_manager') return current;
			const parentId = graph.parentById.get(current.node_id);
			current = parentId === undefined ? undefined : graph.nodesById.get(parentId);
		}
		return null;
	});
	let catalogSequence = $derived(
		session?.getCustomEventSequence(STATE_MACHINE_RUNTIME_PREVIEW_CATALOG_TOPIC) ?? 0
	);
	let catalog = $derived.by((): StateMachinePreviewCatalogDto | null => {
		catalogSequence;
		return (
			session?.getCustomEventPayload<StateMachinePreviewCatalogDto>(
				STATE_MACHINE_RUNTIME_PREVIEW_CATALOG_TOPIC
			) ?? null
		);
	});
	let processorUi = $derived(
		catalog?.processors.find((processor) => processor.id === liveNode.uuid) ?? null
	);
	let mapping = $derived(hasTupleManagedRegions(processorUi));
	let convertNode = $derived(
		processorUi?.standard_mapping && graph
			? directChild(liveNode, 'convert_to_formula', graph.nodesById)
			: null
	);
	let lanes = $derived(
		catalog?.processor_lanes.filter((lane) => lane.processor_id === liveNode.uuid) ?? []
	);
	let selectedContextId = $state('__default__');
	let selectedContext = $derived(
		lanes.find((lane) => contextKeyId(lane.context_key) === selectedContextId)?.context_key ?? null
	);
	let selectedId = $state<string | null>(null);
	let previewStageId = $state<string | null>(null);
	let previewItem = $derived.by((): ManagedItemDto | null => {
		if (!previewStageId) return null;
		return (
			processorUi?.managed_region_instances
				.flatMap((region) => region.items)
				.find((item) => item.id === previewStageId) ?? null
		);
	});
	let previewMode = $derived(
		mapping || !catalog
			? mappingPreviewMode(liveNode.uuid, mapping ? previewItem : null, selectedContext)
			: null
	);
	let previewSequence = $derived(
		session?.getCustomEventSequence(STATE_MACHINE_RUNTIME_PREVIEW_TOPIC) ?? 0
	);
	let preview = $derived.by((): StateMachineRuntimePreviewDto | null => {
		previewSequence;
		return (
			session?.getCustomEventPayload<StateMachineRuntimePreviewDto>(
				STATE_MACHINE_RUNTIME_PREVIEW_TOPIC
			) ?? null
		);
	});
	let sample = $derived.by(() => {
		if (!previewItem) return null;
		for (let index = (preview?.output_preview.length ?? 0) - 1; index >= 0; index--) {
			const candidate = preview?.output_preview[index];
			if (
				candidate?.processor_id === liveNode.uuid &&
				candidate.node_id === previewItem.anode_id &&
				contextKeyId(candidate.context_key) === contextKeyId(selectedContext)
			)
				return candidate;
		}
		return null;
	});
	let busy = $state(false);
	let actionError = $state('');

	$effect(() => {
		const activeSession = session;
		const managerId = manager?.node_id;
		const mode = previewMode;
		if (!activeSession || managerId === undefined || activeSession.status !== 'connected' || !mode)
			return;
		const publish = (next: FormulaPreviewModeDto | null): void => {
			const payload = mappingPreviewDemand(subscriptionId, next);
			void activeSession
				.sendIntent({
					kind: 'sendNodeEvent',
					node: managerId,
					topic: STATE_MACHINE_RUNTIME_PREVIEW_DEMAND_TOPIC,
					payload
				})
				.catch(() => undefined);
		};
		publish(mode);
		const heartbeat = setInterval(() => publish(mode), 2_000);
		return () => {
			clearInterval(heartbeat);
			publish(null);
		};
	});

	const mutate = async (intent: UiEditIntent | UiEditIntent[]): Promise<void> => {
		if (!session || busy) return;
		busy = true;
		actionError = '';
		try {
			if (Array.isArray(intent)) await session.sendIntents(intent);
			else await session.sendIntent(intent);
		} catch (reason) {
			actionError = reason instanceof Error ? reason.message : String(reason);
		} finally {
			busy = false;
		}
	};
	const create = (regionNode: UiNodeDto, item: UiCreatableUserItem): void => {
		void mutate(mappingCreateIntent(regionNode, item));
	};
	const move = (regionNode: UiNodeDto, item: UiNodeDto, direction: -1 | 1): void => {
		const intent = mappingMoveIntent(regionNode, item, direction);
		if (intent) void mutate(intent);
	};
	const duplicate = (regionNode: UiNodeDto, item: UiNodeDto): void => {
		void mutate(mappingDuplicateIntent(regionNode, item));
	};
	const toggle = (item: UiNodeDto): void => {
		void mutate({ kind: 'patchMeta', node: item.node_id, patch: { enabled: !item.meta.enabled } });
	};
	const convert = (nodeId: number): void => {
		const clientEditId = `mapping-conversion:${crypto.randomUUID()}`;
		void mutate([
			{ kind: 'beginEdit', client_edit_id: clientEditId, label: 'Convert Mapping to Formula' },
			{ kind: 'setParam', node: nodeId, value: { kind: 'trigger' }, behaviour: 'Coalesce' },
			{ kind: 'endEdit', client_edit_id: clientEditId }
		]);
	};
	const previewText = (value: unknown): string => {
		const serialized = JSON.stringify(value, (_key, entry) =>
			typeof entry === 'bigint' ? entry.toString() : entry
		);
		return serialized.length > 240 ? `${serialized.slice(0, 240)}…` : serialized;
	};
</script>

{#if mapping && processorUi}
	<section class="mapping-inspector" aria-label="Mapping inspector">
		<header class="mapping-header">
			<strong>{processorUi.label}</strong>
			<span
				>Authored · {busy
					? 'Pending'
					: processorUi.runtime_state === 'active'
						? 'Active runtime'
						: processorUi.runtime_state === 'invalid'
							? 'Invalid runtime'
							: 'Disabled runtime'}</span>
			<button type="button" disabled={busy} onclick={() => void session?.undo()}>Undo</button>
			<button type="button" disabled={busy} onclick={() => void session?.redo()}>Redo</button>
			{#if convertNode}
				<button type="button" disabled={busy} onclick={() => convert(convertNode.node_id)}
					>Convert to Formula</button>
			{/if}
		</header>
		{#if actionError}<p role="alert">{actionError}</p>{/if}
		{#each processorUi.mapping_diagnostics as diagnostic (`${diagnostic.code}:${diagnostic.item_id ?? ''}:${diagnostic.message}`)}
			<p class="diagnostic" role={diagnostic.severity === 'error' ? 'alert' : 'status'}>
				{diagnostic.severity}: {diagnostic.message}
			</p>
		{/each}
		{#each ['input_set', 'filter_pipeline', 'output_set'] as kind (kind)}
			{@const region = processorUi.managed_regions.find((candidate) => candidate.kind === kind)}
			{#if region}
				<MappingRegionList
					{region}
					instance={processorUi.managed_region_instances.find(
						(instance) => instance.region_id === region.id
					) ?? null}
					regionNode={mappingRegionNode(liveNode, region, graph?.nodesById ?? new Map())}
					pipeline={processorUi.mapping_pipeline}
					diagnostics={processorUi.mapping_diagnostics}
					outputs={processorUi.mapping_outputs}
					nodesById={graph?.nodesById ?? new Map()}
					{selectedId}
					{previewStageId}
					{busy}
					onSelect={(id) => (selectedId = id)}
					onPreview={(id) => (previewStageId = previewStageId === id ? null : id)}
					onCreate={create}
					onMove={move}
					onDuplicate={duplicate}
					onRemove={(item) => void mutate({ kind: 'removeNode', node: item.node_id })}
					onToggle={toggle} />
			{/if}
		{/each}
		{#if previewItem}
			<div class="mapping-preview" aria-label="Selected stage preview">
				<label
					>Preview context
					<select bind:value={selectedContextId} aria-label="Preview context">
						<option value="__default__">Default context</option>
						{#each lanes.filter((lane) => lane.context_key !== null) as lane (contextKeyId(lane.context_key))}
							<option value={contextKeyId(lane.context_key)}>{lane.label}</option>
						{/each}
					</select>
				</label>
				<small
					>{sample
						? sample.status === 'live'
							? previewText(sample.value)
							: sample.value.kind === 'string'
								? sample.value.value
								: sample.status
						: 'Waiting for a value in this context'}</small>
			</div>
		{/if}
	</section>
{:else if catalog || !manager || !session || session.status !== 'connected'}
	{@render fallback?.()}
{:else}
	<p class="mapping-loading" role="status">Loading processor…</p>
{/if}

<style>
	.mapping-inspector {
		display: flex;
		flex-direction: column;
		gap: 0.7rem;
		min-inline-size: 0;
		padding: 0.5rem;
	}
	.mapping-header {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.35rem;
	}
	.mapping-header strong {
		flex: 1 1 auto;
	}
	.mapping-header span,
	.mapping-preview small,
	.mapping-loading {
		font-size: 0.72rem;
	}
	button,
	select {
		min-block-size: 1.7rem;
		padding: 0.15rem 0.35rem;
		border: 0.06rem solid var(--gc-color-border);
		border-radius: 0.3rem;
		background: var(--gc-color-background);
		color: var(--gc-color-text);
		font: inherit;
	}
	button {
		cursor: pointer;
	}
	button:disabled {
		opacity: 0.4;
		cursor: default;
	}
	.diagnostic,
	[role='alert'] {
		margin: 0;
		color: var(--gc-color-danger, #e55);
		font-size: 0.72rem;
	}
	.mapping-preview,
	.mapping-preview label {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}
	.mapping-preview {
		padding: 0.45rem;
		border: 0.06rem solid var(--gc-color-border);
		border-radius: 0.35rem;
	}
	.mapping-preview small {
		overflow-wrap: anywhere;
	}
	.mapping-loading {
		margin: 0;
		padding: 0.5rem;
	}
</style>
