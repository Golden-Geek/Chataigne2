<script lang="ts">
	import type { ChannelSnapshot } from '../media/generated/ChannelSnapshot';
	import type { CompositionSnapshot } from '../media/generated/CompositionSnapshot';
	import type { InstanceId } from '../media/generated/InstanceId';
	import type { InstanceReference } from '../media/generated/InstanceReference';
	import type { Point } from '../media/generated/Point';
	import type { InstanceHandle } from './instance-gesture';
	import ChannelPreview from '../media/ChannelPreview.svelte';
	import InstanceHandles from './InstanceHandles.svelte';
	let {
		channel,
		composition,
		selected,
		disabled,
		active,
		select,
		start,
		move,
		finish,
		stop
	}: {
		channel: ChannelSnapshot;
		composition: CompositionSnapshot;
		selected: InstanceId | null;
		disabled: boolean;
		active: boolean;
		select(id: InstanceId): void;
		start(reference: InstanceReference, handle: InstanceHandle): boolean;
		move(point: Point): void;
		finish(): void;
		stop(): void;
	} = $props();
</script>

<div class="preview" style:--preview-aspect={composition.canvas.width / composition.canvas.height}>
	<ChannelPreview {channel} fill />
	<InstanceHandles
		{composition}
		{selected}
		{disabled}
		{active}
		{select}
		{start}
		{move}
		{finish}
		{stop} />
</div>

<style>
	.preview {
		width: min(100%, calc(100cqh * var(--preview-aspect)));
		aspect-ratio: var(--preview-aspect);
		max-height: 100%;
		position: absolute;
		inset: 0;
		margin: auto;
	}
</style>
