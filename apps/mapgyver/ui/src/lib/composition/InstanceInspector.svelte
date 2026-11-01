<script lang="ts">
	import type { Snippet } from 'svelte';
	import { IconButton, type UiNodeDto } from 'golden_ui';
	import copyIcon from 'golden_ui/style/icons/copy.svg';
	import deleteIcon from 'golden_ui/style/icons/delete.svg';
	import { useCompositionAuthoring } from './context.svelte';
	import { useDocumentSelection } from '../selection.svelte';
	let {
		node,
		defaultHeader,
		defaultContent,
		defaultChildren
	}: {
		node: UiNodeDto;
		defaultHeader?: Snippet;
		defaultContent?: Snippet<[Snippet?]>;
		defaultChildren?: Snippet;
	} = $props();
	const owner = useCompositionAuthoring();
	const selection = useDocumentSelection();
	async function duplicate() {
		const selectCreated = selection.capture();
		const result = await owner.controller.edit({
			kind: 'DuplicateInstances',
			instances: [node.uuid]
		});
		if (result?.kind === 'InstancesCreated' && result.instances[0])
			selectCreated(result.instances[0].instance);
	}
	async function remove() {
		const selectParent = selection.capture(),
			parent = owner.view.selected?.channel;
		const result = await owner.controller.edit({ kind: 'RemoveInstance', instance: node.uuid });
		if (result?.kind === 'Changed' && parent) selectParent(parent);
	}
</script>

{@render defaultHeader?.()}
{#snippet content()}
	<div class="actions">
		<IconButton icon={copyIcon} label="Copy instance" disabled={owner.busy} onclick={duplicate} />
		<IconButton icon={deleteIcon} label="Remove instance" disabled={owner.busy} onclick={remove} />
	</div>
	{@render defaultChildren?.()}
{/snippet}
{@render defaultContent?.(content)}

<style>
	.actions {
		display: flex;
		gap: 0.25rem;
		padding: 0.5rem;
	}
</style>
