<script lang="ts">
	import { onDestroy, tick, untrack, type Snippet } from 'svelte';
	import { appState } from '../../../store/workbench.svelte';
	import type { NodeId } from '../../../types';
	import VirtualRows from '../../common/virtual-rows/VirtualRows.svelte';
	import type { OutlinerRow } from './visible-rows';
	import { indexTreeRows, treeNavigationIntent } from './tree-navigation';
	import {
		nodeSelectionMode,
		type NodeActivation,
		type NodeSelectionInput
	} from './node-selection';

	let {
		rows,
		scroller,
		selectedNodeId = null,
		identity = null,
		variableHeights = false,
		onExpand,
		onActivate = null,
		selectable = () => true,
		item: renderItem
	}: {
		rows: readonly OutlinerRow[];
		scroller: HTMLElement | null;
		selectedNodeId?: NodeId | null;
		identity?: unknown;
		variableHeights?: boolean;
		onExpand: (id: NodeId, expanded: boolean) => void;
		onActivate?: NodeActivation | null;
		selectable?: (node: import('../../../types').UiNodeDto) => boolean;
		item: Snippet<[OutlinerRow]>;
	} = $props();
	let virtual = $state<ReturnType<typeof VirtualRows<OutlinerRow>>>();
	let region = $state<HTMLDivElement | null>(null);
	let focusedId = $state<NodeId | null>(null);
	let positions = $derived(indexTreeRows(rows));
	let session = $derived(appState.session);
	let graph = $derived(session?.graph.state ?? null);
	let scope = $derived({ session, generation: session?.snapshotGeneration, identity });
	let activeId = $derived(
		focusedId !== null && positions.has(focusedId)
			? focusedId
			: selectedNodeId !== null && positions.has(selectedNodeId)
				? selectedNodeId
				: (rows[0]?.nodeId ?? null)
	);
	let generation = 0;
	let previousScope: unknown;
	let previousPositions = untrack(() => positions);
	let focusedUuid: string | undefined;
	let disposed = false;

	export const reveal = (id: NodeId, align: 'nearest' | 'center' = 'nearest') =>
		virtual?.reveal(id, align) ?? false;
	export const enter = () => (activeId === null ? Promise.resolve(false) : focusNode(activeId));
	export async function focusNode(id: NodeId): Promise<boolean> {
		const current = ++generation,
			owner = scope,
			uuid = graph?.nodesById.get(id)?.uuid;
		const origin = document.activeElement;
		if (!positions.has(id)) return false;
		focusedId = id;
		focusedUuid = uuid;
		await tick();
		if (disposed || current !== generation || owner !== scope) return false;
		if (!reveal(id)) return false;
		await tick();
		if (
			disposed ||
			current !== generation ||
			owner !== scope ||
			graph?.nodesById.get(id)?.uuid !== uuid ||
			!positions.has(id)
		)
			return false;
		// A newer focus owner (search, another panel or a row control) wins over pending navigation.
		if (
			document.activeElement !== origin &&
			document.activeElement !== scroller &&
			document.activeElement !== document.body
		)
			return false;
		const target = region?.querySelector<HTMLElement>(`[data-tree-node-id="${id}"]`);
		target?.focus({ preventScroll: true });
		return target === document.activeElement;
	}

	const keys = new Set([
		'ArrowUp',
		'ArrowDown',
		'ArrowLeft',
		'ArrowRight',
		'Home',
		'End',
		'Enter',
		' '
	]);
	export async function navigate(
		key: string,
		input: Partial<NodeSelectionInput> = {}
	): Promise<boolean> {
		if (
			!keys.has(key) ||
			((input.ctrlKey || input.metaKey || input.altKey) && key !== ' ' && key !== 'Enter')
		)
			return false;
		const intent = treeNavigationIntent(rows, positions, activeId, key);
		if (!intent) return true;
		if (intent.kind === 'focus') await focusNode(intent.nodeId);
		else if (intent.kind === 'expand') onExpand(intent.nodeId, intent.expanded);
		else {
			const node = graph?.nodesById.get(intent.nodeId);
			if (node && selectable(node)) {
				const modifiers = {
					altKey: input.altKey ?? false,
					ctrlKey: input.ctrlKey ?? false,
					metaKey: input.metaKey ?? false,
					shiftKey: input.shiftKey ?? false,
					detail: input.detail ?? 0
				};
				if (onActivate) onActivate(node, modifiers);
				else session?.selectNode(node.node_id, nodeSelectionMode(modifiers));
			}
		}
		return true;
	}
	const keydown = (event: KeyboardEvent) => {
		const target = event.target;
		if (
			!(target instanceof HTMLElement) ||
			!(
				target.hasAttribute('data-tree-node-id') || target.classList.contains('outliner-item-label')
			)
		)
			return;
		if (
			!keys.has(event.key) ||
			((event.ctrlKey || event.metaKey || event.altKey) &&
				event.key !== 'Enter' &&
				event.key !== ' ')
		)
			return;
		event.preventDefault();
		event.stopPropagation();
		void navigate(event.key, event);
	};
	$effect.pre(() => {
		const next = positions,
			owner = scope,
			nodes = graph?.nodesById;
		untrack(() => {
			const wasInside = region?.contains(document.activeElement) ?? false;
			if (
				previousScope !== owner ||
				(focusedId !== null && nodes?.has(focusedId) && nodes.get(focusedId)?.uuid !== focusedUuid)
			) {
				generation++;
				focusedId = null;
				focusedUuid = undefined;
				if (wasInside) scroller?.focus({ preventScroll: true });
			} else if (focusedId !== null && !next.has(focusedId)) {
				let previous = previousPositions.get(focusedId);
				const oldIndex = previous?.index ?? 0;
				while (previous && !next.has(previous.row.nodeId))
					previous =
						previous.parentId === null ? undefined : previousPositions.get(previous.parentId);
				const fallback =
					previous?.row.nodeId ?? rows[Math.min(oldIndex, rows.length - 1)]?.nodeId ?? null;
				focusedId = fallback;
				focusedUuid = fallback === null ? undefined : nodes?.get(fallback)?.uuid;
				generation++;
				if (wasInside && fallback !== null) void focusNode(fallback);
				else if (wasInside) scroller?.focus({ preventScroll: true });
			}
			previousScope = owner;
			previousPositions = next;
		});
	});
	onDestroy(() => {
		disposed = true;
		generation++;
	});
</script>

<div class="tree-rows" bind:this={region}>
	<VirtualRows
		bind:this={virtual}
		{rows}
		keyOf={(row) => row.nodeId}
		{scroller}
		{variableHeights}
		retainedKeys={activeId === null ? [] : [activeId]}>
		{#snippet item(row)}
			{@const position = positions.get(row.nodeId)!}
			<div
				class="tree-row"
				role="treeitem"
				tabindex={activeId === row.nodeId ? 0 : -1}
				data-tree-node-id={row.nodeId}
				aria-label={graph?.nodesById.get(row.nodeId)?.meta.label}
				aria-level={row.indent + 1}
				aria-expanded={row.expanded}
				aria-selected={selectedNodeId === row.nodeId ||
					(onActivate === null && session?.isNodeSelected(row.nodeId))}
				aria-disabled={graph?.nodesById.has(row.nodeId)
					? !selectable(graph.nodesById.get(row.nodeId)!)
					: true}
				aria-posinset={position.position}
				aria-setsize={position.siblings}
				onkeydown={keydown}
				onfocusin={() => {
					focusedId = row.nodeId;
					focusedUuid = graph?.nodesById.get(row.nodeId)?.uuid;
				}}>
				{@render renderItem(row)}
			</div>
		{/snippet}
	</VirtualRows>
</div>

<style>
	.tree-rows,
	.tree-row {
		inline-size: 100%;
		min-inline-size: 0;
	}
	.tree-row {
		outline-offset: -0.12rem;
	}
	.tree-row:focus-visible {
		outline: solid 0.12rem var(--gc-color-accent, #8aace0);
	}
</style>
