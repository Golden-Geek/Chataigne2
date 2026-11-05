<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { UiNodeDto } from 'golden_ui';
	import MediaInspector from './media/MediaInspector.svelte';
	import ScreenInspector from './screens/ScreenInspector.svelte';
	import ApplicationInspector from './applications/ApplicationInspector.svelte';
	import type { NodeInspectorChildFilter } from 'golden_ui';
	let {
		node,
		defaultHeader,
		defaultContent,
		defaultChildren,
		collapsed = false
	}: {
		node: UiNodeDto;
		defaultHeader?: Snippet<[Snippet?]>;
		defaultContent?: Snippet<[Snippet?]>;
		defaultChildren?: Snippet<[String?, NodeInspectorChildFilter?]>;
		collapsed?: boolean;
	} = $props();
</script>

{#if node.node_type === 'mapgyver.screen' || node.node_type === 'mapgyver.surface'}<ScreenInspector
		{node}
		{defaultHeader}
		{defaultContent}
		{defaultChildren} />
{:else if node.node_type === 'mapgyver.app_control.v1'}<ApplicationInspector
		{node}
		{defaultHeader}
		{defaultContent}
		{defaultChildren} />
{:else}<MediaInspector {node} {collapsed} {defaultHeader} {defaultContent} {defaultChildren} />{/if}
