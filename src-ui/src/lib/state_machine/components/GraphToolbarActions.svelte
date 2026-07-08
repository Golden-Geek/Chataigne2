<script lang="ts">
	import { NodeAddButton, type UiCreatableUserItem, type UiNodeDto } from 'golden_ui';
	import AutoWireToggle from './AutoWireToggle.svelte';

	let {
		autoWire,
		onAutoWireChange,
		addNode = null,
		addItems,
		onCreateItem
	}: {
		autoWire: boolean;
		onAutoWireChange: (enabled: boolean) => void;
		addNode?: UiNodeDto | null;
		addItems?: UiCreatableUserItem[];
		onCreateItem?: (item: UiCreatableUserItem) => void | Promise<void>;
	} = $props();
</script>

<AutoWireToggle checked={autoWire} onchange={onAutoWireChange} />
{#if addNode && onCreateItem}
	{#if addItems === undefined}
		<NodeAddButton node={addNode} {onCreateItem} />
	{:else if addItems.length > 0}
		<NodeAddButton node={addNode} items={addItems} {onCreateItem} />
	{/if}
{/if}
