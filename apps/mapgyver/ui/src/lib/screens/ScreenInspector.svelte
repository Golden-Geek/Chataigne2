<script lang="ts">
	import type { Snippet } from 'svelte';
	import { appState, IconButton, type UiNodeDto, type NodeInspectorChildFilter } from 'golden_ui';
	import { sendCreateUserItemByTypeIntent } from 'golden_ui/store/ui-intents';
	import addIcon from 'golden_ui/style/icons/node/add.svg';
	import { useVideoStatus } from '../runtime/context.svelte';
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
	const runtime = useVideoStatus();
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
	const outputError = $derived(
		ndi && node.meta.enabled
			? runtime?.status?.screens.find((screen) => screen.screen === node.uuid)?.error
			: null
	);
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
	{#if outputError}<p class="output-error" role="status">{outputError}</p>{/if}
	{@render defaultChildren?.('', visibleChild)}
{/snippet}
{@render defaultContent?.(content)}

<style>
	.output-error {
		margin: 0.35rem;
		color: var(--gc-color-warning);
		overflow-wrap: anywhere;
	}
</style>
