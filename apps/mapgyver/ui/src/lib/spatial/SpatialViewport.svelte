<script lang="ts">
	import type { Snippet } from 'svelte';
	import { appState, Viewport2D } from 'golden_ui';
	import ChannelPreview from '../media/ChannelPreview.svelte';
	import type { ChannelSnapshot } from '../media/generated/ChannelSnapshot';
	let {
		channel,
		canvas,
		selection,
		identity,
		label,
		actions,
		children
	}: {
		channel: ChannelSnapshot;
		canvas: { width: number; height: number };
		selection: { x: number; y: number; width: number; height: number } | null;
		identity: string;
		label: string;
		actions?: Snippet;
		children?: Snippet;
	} = $props();
</script>

<Viewport2D
	{actions}
	aspect={canvas.width / canvas.height}
	identity={`${appState.session?.projectGeneration ?? 0}:${identity}`}
	{selection}
	{label}
	native>
	<ChannelPreview {channel} fill />
	{@render children?.()}
</Viewport2D>
