<script lang="ts">
	import type { Snippet } from 'svelte';
	import { appState, IconButton, type UiNodeDto, type NodeInspectorChildFilter } from 'golden_ui';
	import { sendCreateUserItemByTypeIntent } from 'golden_ui/store/ui-intents';
	import addIcon from 'golden_ui/style/icons/node/add.svg';
	let {
		node,
		defaultHeader,
		defaultContent,
		defaultChildren
	}: {
		node: UiNodeDto;
		defaultHeader?: Snippet<[Snippet?]>;
		defaultContent?: Snippet<[Snippet?]>;
		defaultChildren?: Snippet<[String?, NodeInspectorChildFilter?]>;
	} = $props();
	const routeType = 'mapgyver.ndi.audio_route';
	const visibleChild = (child: UiNodeDto) =>
		child.decl_id === 'parameters' || (ndi && child.node_type === routeType);
	const ndi = $derived.by(() => {
		if (node.node_type !== 'mapgyver.screen') return false;
		const session = appState.session;
		const folder = node.children
			.map((id) => session?.getNodeData(id))
			.find((child) => child?.decl_id === 'parameters');
		const target = folder?.children
			.map((id) => session?.getNodeData(id))
			.find((child) => child?.decl_id === 'parameters/target_kind');
		const value = target?.data.kind === 'parameter' ? target.data.param.value : null;
		return value?.kind === 'enum' && value.value === 'NDI';
	});
	let addingRoute = $state(false);
	async function addRoute() {
		addingRoute = true;
		try {
			await sendCreateUserItemByTypeIntent(node.node_id, routeType);
		} finally {
			addingRoute = false;
		}
	}
</script>

{#snippet ndiLink()}
	{#if ndi}
		<a href="https://ndi.video/" target="_blank" rel="noopener noreferrer" aria-label="NDI website">
			NDI
		</a>
		<IconButton
			icon={addIcon}
			label="Add NDI audio route"
			disabled={addingRoute}
			onclick={addRoute} />
	{/if}
{/snippet}
{@render defaultHeader?.(ndi ? ndiLink : undefined)}
{#snippet content()}
	{@render defaultChildren?.('', visibleChild)}
{/snippet}
{@render defaultContent?.(content)}
