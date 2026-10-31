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

<div class="preview">
	<ChannelPreview {channel} />
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
<p>
	Drag an instance or its pivot to move. Use the round handle to rotate and the square handle to
	scale. Forms provide exact values.
</p>

<style>
	.preview {
		position: relative;
	}
	p {
		opacity: 0.8;
		font-size: 0.85em;
	}
</style>
