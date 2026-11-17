<script lang="ts">
	import { viewportContour } from '../../contour';
	import ViewportHandle from '../../ViewportHandle.svelte';
	let selected = $state(false);
	let presses = $state(0);
</script>

<section aria-label="Feedback fixture">
	<button
		onclick={() => {
			selected = !selected;
		}}>Toggle feedback selection</button>
	<output aria-label="Feedback presses">{presses}</output>
	<div class="samples">
		<button aria-label="Feedback contour" use:viewportContour={{ selected, identity: 'paint' }}
			>Object</button>
		<button aria-label="Peer contour" use:viewportContour={{ identity: 'peer' }}>Peer</button>
		<ViewportHandle
			x={0.8}
			y={0.5}
			label="Feedback handle"
			{selected}
			tabindex={0}
			onpointerdown={() => {
				presses++;
			}} />
	</div>
</section>

<style>
	.samples {
		position: relative;
		width: 30rem;
		height: 8rem;
		display: flex;
		gap: 1rem;
		align-items: center;
	}
	.samples > button {
		width: 7rem;
		height: 3rem;
		box-sizing: border-box;
	}
</style>
