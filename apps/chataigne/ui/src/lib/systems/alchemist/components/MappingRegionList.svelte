<script lang="ts">
	import { tick } from 'svelte';
	import { NodeAddButton, type UiCreatableUserItem, type UiNodeDto } from 'golden_ui';
	import type {
		ManagedRegionDefinitionDto,
		ManagedRegionInstanceDto,
		MappingDiagnosticDto,
		MappingOutputTargetDto,
		MappingPipelineShapeDto,
		MappingStageShapeDto
	} from '../../state_machine/generated';
	import { mappingNextIndex, mappingWindow, shapeLabel } from '../mappingInspectorModel';
	import MappingItemEditor from './MappingItemEditor.svelte';

	let {
		region,
		instance,
		regionNode,
		pipeline,
		diagnostics,
		outputs,
		nodesById,
		selectedId,
		previewStageId,
		busy,
		onSelect,
		onPreview,
		onCreate,
		onMove,
		onDuplicate,
		onRemove,
		onToggle
	}: {
		region: ManagedRegionDefinitionDto;
		instance: ManagedRegionInstanceDto | null;
		regionNode: UiNodeDto | null;
		pipeline: MappingPipelineShapeDto | null;
		diagnostics: MappingDiagnosticDto[];
		outputs: MappingOutputTargetDto[];
		nodesById: ReadonlyMap<number, UiNodeDto>;
		selectedId: string | null;
		previewStageId: string | null;
		busy: boolean;
		onSelect: (id: string) => void;
		onPreview: (id: string) => void;
		onCreate: (regionNode: UiNodeDto, item: UiCreatableUserItem) => void;
		onMove: (regionNode: UiNodeDto, item: UiNodeDto, direction: -1 | 1) => void;
		onDuplicate: (regionNode: UiNodeDto, item: UiNodeDto) => void;
		onRemove: (item: UiNodeDto) => void;
		onToggle: (item: UiNodeDto) => void;
	} = $props();

	const ROW_REM = 2.65;
	let listElement: HTMLOListElement | null = $state(null);
	let scrollTop = $state(0);
	let viewportHeight = $state(260);
	let rootFontSize = $state(16);
	let items = $derived(instance?.items ?? []);
	let itemNodes = $derived.by(() => {
		const nodes = new Map<string, UiNodeDto>();
		for (const id of regionNode?.children ?? []) {
			const node = nodesById.get(id);
			if (node) nodes.set(node.uuid, node);
		}
		return nodes;
	});
	let shapes = $derived(new Map(pipeline?.stages.map((stage) => [stage.item_id, stage]) ?? []));
	let outputTargets = $derived(new Map(outputs.map((target) => [target.item_id, target])));
	let window = $derived(
		mappingWindow(items.length, scrollTop, viewportHeight, ROW_REM * rootFontSize)
	);
	let visible = $derived(items.slice(window.start, window.end));
	let selectedItem = $derived(items.find((item) => item.id === selectedId) ?? null);
	let selectedNode = $derived(selectedItem ? (itemNodes.get(selectedItem.id) ?? null) : null);
	let selectedOutputTarget = $derived(
		selectedItem ? (outputTargets.get(selectedItem.id) ?? null) : null
	);

	const focusItem = async (index: number): Promise<void> => {
		const next = items[index];
		if (!next || !listElement) return;
		onSelect(next.id);
		const rowHeight = ROW_REM * rootFontSize;
		const top = index * rowHeight;
		if (top < listElement.scrollTop) listElement.scrollTop = top;
		if (top + rowHeight > listElement.scrollTop + listElement.clientHeight) {
			listElement.scrollTop = top + rowHeight - listElement.clientHeight;
		}
		scrollTop = listElement.scrollTop;
		await tick();
		listElement.querySelector<HTMLButtonElement>(`button[data-item-id="${next.id}"]`)?.focus();
	};

	const onRowKeydown = (event: KeyboardEvent, index: number): void => {
		if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
			event.preventDefault();
			void focusItem(mappingNextIndex(items.length, index, event.key === 'ArrowDown' ? 1 : -1));
		}
	};

	$effect(() => {
		if (!listElement) return;
		const update = (): void => {
			viewportHeight = listElement?.clientHeight ?? 260;
			rootFontSize = Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
		};
		update();
		const observer = new ResizeObserver(update);
		observer.observe(listElement);
		return () => observer.disconnect();
	});
</script>

<section class="mapping-region" aria-label={region.label}>
	<header>
		<strong>{region.label}</strong>
		{#if regionNode}
			<NodeAddButton
				node={regionNode}
				items={regionNode.creatable_user_items}
				onCreateItem={(item) => onCreate(regionNode, item)} />
		{/if}
	</header>
	{#if region.kind === 'input_set'}
		<p class="shape-summary">Input value: {shapeLabel(pipeline?.input ?? null)}</p>
	{:else if region.kind === 'output_set'}
		<p class="shape-summary">Result value: {shapeLabel(pipeline?.output ?? null)}</p>
	{/if}
	{#if items.length > 0}
		<ol
			bind:this={listElement}
			class="mapping-items"
			aria-label={`${region.label} items`}
			onscroll={(event) => (scrollTop = event.currentTarget.scrollTop)}>
			{#if window.start > 0}<li
					class="spacer"
					style:height={`${window.start * ROW_REM}rem`}
					aria-hidden="true">
				</li>{/if}
			{#each visible as item, visibleIndex (item.id)}
				{@const index = window.start + visibleIndex}
				{@const itemNode = itemNodes.get(item.id)}
				{@const stageShape = shapes.get(item.id) as MappingStageShapeDto | undefined}
				{@const itemDiagnostics = diagnostics.filter(
					(diagnostic) => diagnostic.item_id === item.id || diagnostic.item_id === item.anode_id
				)}
				<li
					class="mapping-row"
					class:selected={selectedId === item.id}
					class:off={!item.enabled || !item.anode_enabled}
					aria-posinset={index + 1}
					aria-setsize={items.length}>
					<button
						type="button"
						class="row-main"
						data-item-id={item.id}
						aria-label={`${item.label}, item ${index + 1} of ${items.length}`}
						aria-pressed={selectedId === item.id}
						onclick={() => onSelect(item.id)}
						onkeydown={(event) => onRowKeydown(event, index)}>
						<span>{item.label}</span>
						{#if region.kind === 'filter_pipeline'}
							<small
								>{shapeLabel(stageShape?.before ?? null)} → {shapeLabel(
									stageShape?.after ?? null
								)}</small>
						{/if}
					</button>
					{#if itemDiagnostics.length > 0}<span
							class="row-warning"
							title={itemDiagnostics.map((entry) => entry.message).join('\n')}>!</span
						>{/if}
					{#if region.kind === 'filter_pipeline'}
						<button
							type="button"
							class="row-action"
							aria-label={`Preview ${item.label}`}
							aria-pressed={previewStageId === item.id}
							onclick={() => onPreview(item.id)}>◉</button>
					{/if}
					{#if itemNode && regionNode}
						<button
							type="button"
							class="row-action"
							aria-label={`Move ${item.label} up`}
							disabled={busy || index === 0}
							onclick={() => onMove(regionNode, itemNode, -1)}>↑</button>
						<button
							type="button"
							class="row-action"
							aria-label={`Move ${item.label} down`}
							disabled={busy || index === items.length - 1}
							onclick={() => onMove(regionNode, itemNode, 1)}>↓</button>
						<button
							type="button"
							class="row-action"
							aria-label={`${item.enabled ? 'Disable' : 'Enable'} ${item.label}`}
							disabled={busy}
							onclick={() => onToggle(itemNode)}>{item.enabled ? 'On' : 'Off'}</button>
						<button
							type="button"
							class="row-action"
							aria-label={`Duplicate ${item.label}`}
							disabled={busy}
							onclick={() => onDuplicate(regionNode, itemNode)}>⧉</button>
						<button
							type="button"
							class="row-action"
							aria-label={`Remove ${item.label}`}
							disabled={busy}
							onclick={() => onRemove(itemNode)}>×</button>
					{/if}
				</li>
			{/each}
			{#if window.end < items.length}<li
					class="spacer"
					style:height={`${(items.length - window.end) * ROW_REM}rem`}
					aria-hidden="true">
				</li>{/if}
		</ol>
		{#if selectedNode && (region.kind === 'input_set' || region.kind === 'filter_pipeline' || region.kind === 'output_set')}
			<MappingItemEditor
				node={selectedNode}
				kind={region.kind}
				outputShape={pipeline?.output ?? null}
				outputTarget={selectedOutputTarget}
				{nodesById} />
		{/if}
	{:else}
		<p class="empty">Add an item to begin.</p>
	{/if}
</section>

<style>
	.mapping-region {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
		min-inline-size: 0;
	}
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.35rem;
	}
	header strong {
		font-size: 0.84rem;
	}
	.shape-summary,
	.empty {
		margin: 0;
		color: color-mix(in srgb, var(--gc-color-text) 75%, transparent);
		font-size: 0.72rem;
	}
	.mapping-items {
		max-block-size: 15.9rem;
		margin: 0;
		padding: 0;
		overflow: auto;
		list-style: none;
		border: 0.06rem solid var(--gc-color-border);
		border-radius: 0.35rem;
	}
	.spacer {
		display: block;
	}
	.mapping-row {
		display: flex;
		align-items: center;
		gap: 0.15rem;
		box-sizing: border-box;
		block-size: 2.65rem;
		padding: 0 0.22rem;
		border-block-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
	}
	.mapping-row.selected {
		background: color-mix(in srgb, var(--gc-color-accent) 16%, transparent);
	}
	.mapping-row.off .row-main {
		opacity: 0.58;
	}
	.row-main {
		display: flex;
		flex-direction: column;
		justify-content: center;
		flex: 1 1 auto;
		min-inline-size: 0;
		block-size: 100%;
		padding: 0.1rem 0.15rem;
		border: 0;
		background: transparent;
		color: var(--gc-color-text);
		text-align: start;
		cursor: pointer;
	}
	.row-main span,
	.row-main small {
		display: block;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.row-main span {
		font-size: 0.72rem;
		font-weight: 650;
	}
	.row-main small {
		font-size: 0.62rem;
		opacity: 0.72;
	}
	.row-action {
		flex: 0 0 auto;
		min-inline-size: 1.3rem;
		block-size: 1.5rem;
		padding: 0 0.1rem;
		border: 0;
		border-radius: 0.22rem;
		background: transparent;
		color: var(--gc-color-text);
		font: inherit;
		font-size: 0.7rem;
		cursor: pointer;
	}
	.row-action:hover,
	.row-action[aria-pressed='true'] {
		background: color-mix(in srgb, var(--gc-color-accent) 25%, transparent);
	}
	.row-action:disabled {
		opacity: 0.32;
		cursor: default;
	}
	.row-warning {
		color: var(--gc-color-danger, #e55);
		font-weight: 800;
	}
</style>
