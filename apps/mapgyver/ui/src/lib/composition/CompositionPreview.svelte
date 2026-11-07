<script lang="ts">
	import type { Snippet } from 'svelte';
	import { pointBounds } from 'golden_ui';
	import SpatialViewport from '../spatial/SpatialViewport.svelte';
	import type { ChannelSnapshot } from '../media/generated/ChannelSnapshot';
	import type { CompositionSnapshot } from '../media/generated/CompositionSnapshot';
	import type { InstanceId } from '../media/generated/InstanceId';
	import type { InstanceReference } from '../media/generated/InstanceReference';
	import type { Point } from '../media/generated/Point';
	import type { InstanceHandle, InstancePresentation } from './instance-gesture';
	import InstanceHandles from './InstanceHandles.svelte';
	import { usePreview } from '../preview/context.svelte';
	import { previewKey } from '../preview/identity';
	import { instanceDisplay, gizmo } from './presentation';
	import type { SpatialKeyboardMode } from '../spatial/keyboard.svelte';
	let {
		channel,
		composition,
		presentation = null,
		selected,
		disabled,
		active,
		select,
		start,
		move,
		finish,
		stop,
		actions
	}: {
		channel: ChannelSnapshot;
		composition: CompositionSnapshot;
		presentation?: InstancePresentation | null;
		selected: InstanceId | null;
		disabled: boolean;
		active: boolean;
		select(id: InstanceId): void;
		start(reference: InstanceReference, handle: InstanceHandle): boolean;
		move(point: Point): void;
		finish(): void;
		stop(): void;
		actions?: Snippet;
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
						instanceDisplay(instance, extent, presentation).points.map((point) => ({
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
	let keyboardOrigin: Point = { x: 0, y: 0 };
	function keyboardStart(mode: SpatialKeyboardMode) {
		if (!instance) return false;
		const origin = instance.definition.transform;
		let geometryMode: InstanceHandle['mode'] = 'Translate';
		keyboardOrigin = { x: 0, y: 0 };
		if (typeof mode !== 'string') {
			if (!extent) return false;
			const handles = gizmo(origin, extent);
			const handle =
				'Resize' in mode
					? handles.resize.find((handle) => handle.name === mode.Resize.handle)
					: handles.rotate.find((handle) => handle.name === mode.Rotate.handle);
			if (!handle) return false;
			keyboardOrigin = { ...handle.point };
			geometryMode =
				'Resize' in mode
					? { Resize: { handle: mode.Resize.handle, extent: { ...extent } } }
					: 'Rotate';
		}
		const id = instance.reference.instance;
		const admitted = start(instance.reference, {
			mode: geometryMode,
			origin: {
				...origin,
				position: [...origin.position],
				scale: [...origin.scale],
				pivot: [...origin.pivot]
			},
			start: { ...keyboardOrigin }
		});
		if (admitted) select(id);
		return admitted;
	}
</script>

<SpatialViewport
	{actions}
	{channel}
	canvas={composition.canvas}
	identity={channel.reference.channel}
	{selection}
	keyboard={{
		id: instance?.reference.instance ?? null,
		disabled,
		active,
		start: keyboardStart,
		move: (point) => move({ x: keyboardOrigin.x + point.x, y: keyboardOrigin.y + point.y }),
		finish,
		stop
	}}
	label="Composition view">
	<InstanceHandles
		{presentation}
		{composition}
		{selected}
		{disabled}
		{active}
		{select}
		{start}
		{move}
		{finish}
		{stop} />
</SpatialViewport>
