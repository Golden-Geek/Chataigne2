<script lang="ts">
	import { Viewport2D } from 'golden_ui';
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
	let inspected = $state('screen');
	let edits = $state(0);
	function select(id: string | null) {
		selected = id;
		inspected = id ?? screen.id;
	}
</script>

<output aria-label="Inspected owner">{inspected}</output>
<output aria-label="Geometry starts">{edits}</output>
<main>
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
</main>

<style>
	main {
		width: 36rem;
		height: 30rem;
	}
</style>
