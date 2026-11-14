<script lang="ts">
	import type { Snippet } from 'svelte';
	import { appState, Viewport2D } from 'golden_ui';
	import ChannelPreview from '../media/ChannelPreview.svelte';
	import type { ChannelSnapshot } from '../media/generated/ChannelSnapshot';
	import type { PreviewSource } from '../preview/generated/PreviewSource';
	import { createSpatialKeyboard, type SpatialKeyboardClient } from './keyboard.svelte';
	let {
		channel,
		canvas,
		selection,
		identity,
		label,
		actions,
		children,
		preview,
		previewLabel,
		onselectionclear,
		keyboard: keyboardClient
	}: {
		channel: ChannelSnapshot;
		canvas: { width: number; height: number };
		selection: { x: number; y: number; width: number; height: number } | null;
		identity: string;
		label: string;
		actions?: Snippet;
		children?: Snippet;
		preview?: PreviewSource | null;
		previewLabel?: string;
		onselectionclear?(): void;
		keyboard?: SpatialKeyboardClient;
	} = $props();
	const keyboard = createSpatialKeyboard(() => keyboardClient);
</script>

<svelte:window onkeyup={keyboard.keyup} onblur={keyboard.finish} />

<Viewport2D
	{actions}
	aspect={canvas.width / canvas.height}
	identity={`${appState.session?.projectGeneration ?? 0}:${identity}`}
	{selection}
	{label}
	{onselectionclear}
	onkeydown={keyboard.keydown}
	onfocusout={keyboard.finish}
	native>
	{#if preview !== null}<ChannelPreview
			{channel}
			fill
			{preview}
			label={previewLabel}
			extent={preview ? canvas : undefined} />{/if}
	{@render children?.()}
</Viewport2D>
