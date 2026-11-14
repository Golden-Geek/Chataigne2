<script lang="ts">
	import { onMount } from 'svelte';
	import { Viewport2D, selectedEditorItem } from 'golden_ui';
	import SpatialViewport from '../../../spatial/SpatialViewport.svelte';
	import RectLayoutControls from '../../../spatial/RectLayoutControls.svelte';
	import type { LayoutRect } from '../../../spatial/rect-presentation';
	import CompositionPreview from '../../../composition/CompositionPreview.svelte';
	import { providePreview } from '../../../preview/context.svelte';
	import type { CompositionSnapshot } from '../../../media/generated/CompositionSnapshot';
	import type { ChannelSnapshot } from '../../../media/generated/ChannelSnapshot';
	import type { InstanceSnapshot } from '../../../media/generated/InstanceSnapshot';
	import SurfaceHandles from '../../SurfaceHandles.svelte';
	import type { ScreenSnapshot } from '../../generated/ScreenSnapshot';
	import type { SurfaceSnapshot } from '../../generated/SurfaceSnapshot';
	const screen: ScreenSnapshot = {
		id: 'screen',
		label: 'Screen',
		definition: {
			target: null,
			enabled: true,
			anti_aliasing: 'Raw',
			frame_rate: { kind: 'FollowDisplay' }
		},
		surfaces: ['A', 'B'].map((id, index): SurfaceSnapshot => ({
			reference: { screen: 'screen', surface: id },
			label: id,
			mapping_valid: index === 0,
			definition: {
				source: null,
				crop: { x: 0, y: 0, width: 1, height: 1 },
				quad: {
					corners: [
						{ x: 0.1 + index * 0.45, y: 0.2 },
						{ x: 0.4 + index * 0.45, y: 0.2 },
						{ x: 0.4 + index * 0.45, y: 0.8 },
						{ x: 0.1 + index * 0.45, y: 0.8 }
					]
				},
				opacity: 1,
				calibration: 'Off',
				enabled: true
			}
		}))
	};
	let selected = $state<string | null>(null);
	let compositionMode = $state(false);
	let matrixKind = $state<'Layer' | 'Cell' | null>(null);
	const preview = providePreview();
	onMount(preview.mount);
	const channel: ChannelSnapshot = {
		reference: { media: 'composition', channel: 'composition-output' },
		label: 'Composition',
		definition: {
			source: { kind: 'Composition', canvas: { width: 400, height: 300 } },
			size: { kind: 'Automatic' },
			fit_mode: 'Fit'
		}
	};
	const composition: CompositionSnapshot = {
		reference: channel.reference,
		canvas: { width: 400, height: 300 },
		instances: ['Item A', 'Item B'].map((label, index): InstanceSnapshot => ({
			reference: { composition: channel.reference, instance: label },
			label,
			definition: {
				source: null,
				opacity: 1,
				blend: 'Normal',
				enabled: true,
				transform: {
					position: [100 + index * 200, 150],
					scale: [1, 1],
					pivot: [0.5, 0.5],
					rotation_degrees: 0
				}
			}
		}))
	};
	const matrixChannel: ChannelSnapshot = {
		reference: { media: 'matrix', channel: 'matrix-output' },
		label: 'Matrix',
		definition: {
			source: {
				kind: 'Matrix',
				output: {
					kind: 'Final',
					definition: { canvas: composition.canvas, empty_cell: 'Transparent' }
				}
			},
			size: { kind: 'Automatic' },
			fit_mode: 'Fit'
		}
	};
	const rectangles: LayoutRect[] = ['Rect A', 'Rect B'].map((id, index) => ({
		id,
		label: id,
		position: [40 + index * 200, 80],
		size: { width: 80, height: 60 },
		enabled: true
	}));
	const matrixSelected = $derived(
		selectedEditorItem(selected, rectangles, rectangles[0], (item) => item.id)
	);
	let inspected = $state('screen');
	let edits = $state(0);
	function select(id: string | null) {
		selected = id;
		inspected = id ?? screen.id;
	}
</script>

<output aria-label="Inspected owner">{inspected}</output>
<output aria-label="Geometry starts">{edits}</output>
<button
	onclick={() => {
		compositionMode = true;
		matrixKind = null;
		selected = null;
		inspected = 'composition';
	}}>Show Composition</button>
{#each ['Layer', 'Cell'] as kind}
	<button
		onclick={() => {
			matrixKind = kind === 'Layer' ? 'Layer' : 'Cell';
			selected = null;
			inspected = matrixKind === 'Layer' ? 'matrix' : 'layer-owner';
		}}>Show Matrix {kind}</button>
{/each}
<main>
	{#if matrixKind}
		<SpatialViewport
			channel={matrixChannel}
			canvas={composition.canvas}
			selection={null}
			identity={matrixKind}
			label="Matrix fixture"
			preview={null}
			onselectionclear={() => {
				selected = null;
				inspected = matrixKind === 'Layer' ? 'matrix' : 'layer-owner';
			}}>
			<RectLayoutControls
				objects={rectangles}
				canvas={composition.canvas}
				kind={matrixKind}
				selected={matrixSelected?.id ?? null}
				disabled={false}
				active={false}
				select={(id) => {
					selected = id;
					inspected = id;
				}}
				start={() => false}
				move={() => {}}
				finish={() => {}}
				stop={() => {}} />
		</SpatialViewport>
	{:else if compositionMode}
		<CompositionPreview
			{channel}
			{composition}
			{selected}
			disabled={false}
			active={false}
			select={(id) => {
				selected = id;
				inspected = id;
			}}
			onselectionclear={() => {
				selected = null;
				inspected = 'composition';
			}}
			start={() => false}
			move={() => {}}
			finish={() => {}}
			stop={() => {}} />
	{:else}
		<Viewport2D
			aspect={1}
			identity={screen.id}
			label="Warp fixture"
			onselectionclear={() => select(null)}>
			<SurfaceHandles
				{screen}
				{selected}
				selectedCorner={null}
				disabled={false}
				{select}
				pickCorner={(id) => select(id)}
				start={() => {
					edits++;
					return true;
				}}
				move={() => {}}
				finish={() => {}}
				stop={() => {}} />
		</Viewport2D>
	{/if}
</main>

<style>
	main {
		width: 36rem;
		height: 30rem;
	}
</style>
