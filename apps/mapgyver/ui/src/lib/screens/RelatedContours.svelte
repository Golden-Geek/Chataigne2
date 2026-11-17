<script lang="ts">
	import { viewportContour } from 'golden_ui';
	import type { OverlayPrimitive } from 'golden_video_ui';
	import type { RelatedSurface } from './generated/RelatedSurface';
	import { overlayRem, quadOverlay } from './overlays';
	import { cropCorners } from './gesture-presentation';
	let {
		surfaces,
		available,
		selected,
		select,
		onoverlays
	}: {
		surfaces: RelatedSurface[];
		available: boolean;
		selected: string | null;
		select(uuid: string): void;
		onoverlays(overlays: OverlayPrimitive[]): void;
	} = $props();
	let hovered = $state<string | null>(null),
		focused = $state<string | null>(null);
	const isSelected = (entry: RelatedSurface) =>
		selected === entry.surface.reference.surface ||
		[entry.parameters.crop_origin, entry.parameters.crop_size].some((id) => id === selected);
	$effect(() => {
		onoverlays(
			surfaces.map((entry) =>
				quadOverlay(cropCorners(entry.surface, null), overlayRem(), {
					hit: `related:${entry.surface.reference.surface}`,
					selected: isSelected(entry),
					hovered:
						hovered === entry.surface.reference.surface ||
						focused === entry.surface.reference.surface,
					enabled: entry.surface.definition.enabled
				})
			)
		);
	});
</script>

<svg
	data-native-overlay-control
	viewBox="0 0 1 1"
	preserveAspectRatio="none"
	role="group"
	aria-label="Related source Surfaces">
	{#each surfaces as entry (entry.surface.reference.surface)}
		{@const { surface, screen_label } = entry}
		<rect
			use:viewportContour={{
				selected: isSelected(entry),
				enabled: surface.definition.enabled,
				onhover: (value) => {
					hovered = value ? surface.reference.surface : null;
				},
				onfocus: (value) => {
					focused = value ? surface.reference.surface : null;
				}
			}}
			data-native-hit-id={`related:${surface.reference.surface}`}
			data-related-surface={surface.reference.surface}
			role="button"
			tabindex={available ? 0 : -1}
			aria-disabled={!available}
			aria-pressed={isSelected(entry)}
			aria-label={`Select ${surface.label} on ${screen_label}`}
			x={surface.definition.crop.x}
			y={surface.definition.crop.y}
			width={surface.definition.crop.width}
			height={surface.definition.crop.height}
			onpointerdown={(event) => {
				if (!available || event.button !== 0 || event.defaultPrevented) return;
				event.preventDefault();
				event.stopPropagation();
				select(surface.reference.surface);
			}}
			onkeydown={(event) => {
				if (!available || (event.key !== 'Enter' && event.key !== ' ')) return;
				event.preventDefault();
				event.stopPropagation();
				select(surface.reference.surface);
			}}
			onclick={(event) => {
				if (available && event.detail === 0) select(surface.reference.surface);
			}}>
			<title>{surface.label} · {screen_label}</title>
		</rect>
	{/each}
</svg>

<style>
	svg {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		overflow: visible;
		pointer-events: none;
	}
	rect {
		pointer-events: all;
		cursor: pointer;
	}
</style>
