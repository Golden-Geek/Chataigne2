<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { UiNodeDto } from 'golden_ui';
	import MediaInspector from './media/MediaInspector.svelte';
	import ScreenInspector from './screens/ScreenInspector.svelte';
	import MatrixProperties from './matrix/MatrixProperties.svelte';
	import ApplicationInspector from './applications/ApplicationInspector.svelte';
	import type { NodeInspectorChildFilter } from 'golden_ui';
	let {
		node,
		defaultHeader,
		defaultContent,
		defaultChildren
	}: {
		node: UiNodeDto;
		defaultHeader?: Snippet;
		defaultContent?: Snippet<[Snippet?]>;
		defaultChildren?: Snippet<[String?, NodeInspectorChildFilter?]>;
	} = $props();
</script>

{#if node.node_type === 'mapgyver.screen' || node.node_type === 'mapgyver.surface'}<ScreenInspector
		{node}
		{defaultHeader}
		{defaultContent}
		{defaultChildren} />
{:else if node.node_type.startsWith('mapgyver.matrix.')}{@render defaultHeader?.()}<MatrixProperties />
{:else if node.node_type === 'mapgyver.application'}<ApplicationInspector {node} {defaultHeader} />
{:else}<MediaInspector {node} {defaultHeader} {defaultContent} {defaultChildren} />{/if}
