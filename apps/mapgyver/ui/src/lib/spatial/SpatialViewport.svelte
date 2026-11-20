<script lang="ts">
	import type { Snippet } from 'svelte';
	import { appState, Viewport2D, type UiBrowserPreviewDependency } from 'golden_ui';
	import { hasDesktopHost } from 'golden_ui/host/desktop';
	import { MAX_VIEW_OVERLAYS, type OverlayPrimitive } from 'golden_video_ui';
	import ChannelPreview from '../media/ChannelPreview.svelte';
	import type { ChannelSnapshot } from '../media/generated/ChannelSnapshot';
	import type { PreviewSource } from '../preview/generated/PreviewSource';
	import { createSpatialKeyboard, type SpatialKeyboardClient } from './keyboard.svelte';
	import { provideSpatialPaint } from './native-paint';
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
		ondependencies,
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
		ondependencies?(dependencies: readonly UiBrowserPreviewDependency[]): void;
		onselectionclear?(): void;
		keyboard?: SpatialKeyboardClient;
	} = $props();
	const keyboard = createSpatialKeyboard(() => keyboardClient);
	const native = hasDesktopHost();
	let overlays = $state.raw<OverlayPrimitive[]>([]);
	let painted = $state(false);
	const nativeFrame = $derived(preview !== null && painted);
	provideSpatialPaint(
		native
			? (next) => {
					overlays = next;
				}
			: null
	);
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
	{native}
	{nativeFrame}>
	{#if preview !== null}<ChannelPreview
			{channel}
			fill
			{preview}
			{ondependencies}
			{overlays}
			overlayCapacity={native ? MAX_VIEW_OVERLAYS : 0}
			onpresentationchange={(value) => {
				painted = value;
			}}
			label={previewLabel}
			extent={canvas} />{/if}
	{@render children?.()}
</Viewport2D>

<style>
	:global(.active-zone.native-frame [data-native-overlay-control]) {
		opacity: 0;
	}
	:global(.active-zone.native-frame [data-native-overlay-control]:focus-visible) {
		opacity: 1;
	}
</style>
