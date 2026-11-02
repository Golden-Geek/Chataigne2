<script lang="ts">
	import { Viewport2D, pointBounds } from 'golden_ui';
	import type { ChannelSnapshot } from '../media/generated/ChannelSnapshot';
	import type { CompositionSnapshot } from '../media/generated/CompositionSnapshot';
	import type { InstanceId } from '../media/generated/InstanceId';
	import type { InstanceReference } from '../media/generated/InstanceReference';
	import type { Point } from '../media/generated/Point';
	import type { InstanceHandle } from './instance-gesture';
	import ChannelPreview from '../media/ChannelPreview.svelte';
	import InstanceHandles from './InstanceHandles.svelte';
	import { usePreview } from '../preview/context.svelte';
	import { previewKey } from '../preview/identity';
	import { outline } from './presentation';
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
	const owner = usePreview();
	const instance = $derived(
		composition.instances.find((item) => item.reference.instance === selected)
	);
	const extent = $derived(
		instance?.definition.source
			? owner.statuses.get(previewKey({ kind: 'Channel', reference: instance.definition.source }))
					?.extent
			: null
	);
	const selection = $derived(
		instance
			? extent
				? pointBounds(
						outline(instance.definition.transform, extent).map((point) => ({
							x: point.x / composition.canvas.width,
							y: point.y / composition.canvas.height
						}))
					)
				: {
						x: instance.definition.transform.position[0] / composition.canvas.width - 0.05,
						y: instance.definition.transform.position[1] / composition.canvas.height - 0.05,
						width: 0.1,
						height: 0.1
					}
			: null
	);
</script>

<Viewport2D
	aspect={composition.canvas.width / composition.canvas.height}
	identity={channel.reference.channel}
	{selection}
	label="Composition view"
	native>
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
</Viewport2D>
