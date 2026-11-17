<script lang="ts">
	import { onMount } from 'svelte';
	import GridFixture from '../../../matrix/tests/browser/GridFixture.svelte';
	import { Viewport2D, selectedEditorItem } from 'golden_ui';
	import SpatialViewport from '../../../spatial/SpatialViewport.svelte';
	import RectLayoutControls from '../../../spatial/RectLayoutControls.svelte';
	import type { LayoutRect } from '../../../spatial/rect-presentation';
	import CompositionPreview from '../../../composition/CompositionPreview.svelte';
	import { providePreview } from '../../../preview/context.svelte';
	import { previewKey } from '../../../preview/identity';
	import type { CompositionSnapshot } from '../../../media/generated/CompositionSnapshot';
	import type { ChannelSnapshot } from '../../../media/generated/ChannelSnapshot';
	import type { InstanceSnapshot } from '../../../media/generated/InstanceSnapshot';
	import SurfaceHandles from '../../SurfaceHandles.svelte';
	import SurfaceSourcePreview from '../../SurfaceSourcePreview.svelte';
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
	let extentAvailable = $state(false);
	let nativeOverlays = $state('[]');
	let matrixKind = $state<'Layer' | 'Cell' | null>(null);
	let sourceMode = $state(false);
	let gridMode = $state(false);
	let cropEditing = $state(false);
	let cropBusy = $state(false);
	let cropInputAvailable = $state(true);
	let warpBusy = $state(false);
	let warpSelectionAvailable = $state(true);
	let warpCorner = $state<number | null>(null);
	let spatialBusy = $state(false);
	let cropFinishes = $state(0),
		cropStops = $state(0),
		cropMoves = $state(0);
	const cropSurface: SurfaceSnapshot = {
		...screen.surfaces[0],
		definition: {
			...screen.surfaces[0].definition,
			crop: { x: 0.2, y: 0.2, width: 0.6, height: 0.6 }
		}
	};
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
	preview.statuses.set(previewKey({ kind: 'Channel', reference: channel.reference }), {
		source: { kind: 'Channel', reference: channel.reference },
		extent: { width: 80, height: 60 },
		error: null,
		message: null,
		matrix: null
	});
	const shownComposition = $derived({
		...composition,
		instances: composition.instances.map((item) => ({
			...item,
			definition: { ...item.definition, source: extentAvailable ? channel.reference : null }
		}))
	});
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
	rectangles.push({
		id: 'Missing bounds',
		label: 'Missing bounds',
		position: [180, 30],
		size: null,
		enabled: false
	});
	let inspected = $state('screen');
	let edits = $state(0);
	function select(id: string | null) {
		if (id !== selected) warpCorner = null;
		selected = id;
		inspected = id ?? screen.id;
	}
</script>

<div class="fixture-toolbar">
	<output aria-label="Inspected owner">{inspected}</output>
	<button
		onclick={() => {
			gridMode = true;
		}}>Show Matrix grid</button>
	<output aria-label="Native overlays" hidden>{nativeOverlays}</output>
	<button
		onclick={() => {
			extentAvailable = !extentAvailable;
		}}>Toggle source bounds</button>
	<output aria-label="Geometry starts">{edits}</output>
	<button onclick={() => (spatialBusy = !spatialBusy)}>Toggle spatial metadata busy</button>
	<button onclick={() => (warpBusy = !warpBusy)}>Toggle warp metadata busy</button>
	<button onclick={() => (cropInputAvailable = !cropInputAvailable)}
		>Toggle crop input availability</button>
	<button onclick={() => (warpSelectionAvailable = !warpSelectionAvailable)}>
		Toggle warp input availability
	</button>
	<output aria-label="Crop finishes">{cropFinishes}</output>
	<output aria-label="Crop stops">{cropStops}</output>
	<output aria-label="Crop moves">{cropMoves}</output>
	<button
		onclick={() => {
			sourceMode = true;
			gridMode = false;
			selected = null;
			inspected = 'A';
		}}>Show Surface Source</button>
	<button
		onclick={() => {
			cropEditing = !cropEditing;
		}}>Toggle crop editing</button>
	<button
		onclick={() => {
			cropBusy = !cropBusy;
		}}>Toggle crop metadata busy</button>
	<button
		onclick={() => {
			sourceMode = false;
			compositionMode = true;
			matrixKind = null;
			selected = null;
			inspected = 'composition';
		}}>Show Composition</button>
	{#each ['Layer', 'Cell'] as kind}
		<button
			onclick={() => {
				sourceMode = false;
				matrixKind = kind === 'Layer' ? 'Layer' : 'Cell';
				selected = null;
				inspected = matrixKind === 'Layer' ? 'matrix' : 'layer-owner';
			}}>Show Matrix {kind}</button>
	{/each}
</div>
<main>
	{#if gridMode}
		<GridFixture
			onselect={(id) => {
				inspected = id;
			}} />
	{:else if sourceMode}
		<SurfaceSourcePreview
			inputAvailable={cropInputAvailable}
			surface={cropSurface}
			selected={selected === 'crop-origin' || selected === 'crop-size'}
			disabled={cropBusy}
			select={(handle) => {
				selected = handle === 'origin' ? 'crop-origin' : 'crop-size';
				inspected = selected;
				return true;
			}}
			onselectionclear={() => {
				selected = null;
				inspected = 'A';
			}}
			start={() => {
				if (!cropEditing) return false;
				edits++;
				return true;
			}}
			move={() => {
				cropMoves++;
			}}
			finish={() => {
				cropFinishes++;
			}}
			stop={() => {
				cropStops++;
			}} />
	{:else if matrixKind}
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
				identity={matrixKind}
				objects={rectangles}
				canvas={composition.canvas}
				kind={matrixKind}
				selected={matrixSelected?.id ?? null}
				disabled={spatialBusy}
				selectionAvailable={true}
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
			composition={shownComposition}
			{selected}
			disabled={spatialBusy}
			selectionAvailable={true}
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
				onoverlays={(value) => {
					nativeOverlays = JSON.stringify(value);
				}}
				{screen}
				{selected}
				selectedCorner={warpCorner}
				disabled={warpBusy}
				selectionAvailable={warpSelectionAvailable}
				{select}
				pickCorner={(id, corner) => {
					select(id);
					warpCorner = corner;
				}}
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
	.fixture-toolbar {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
	}
	output {
		white-space: nowrap;
		overflow: hidden;
	}
	main {
		width: 36rem;
		height: 30rem;
	}
</style>
