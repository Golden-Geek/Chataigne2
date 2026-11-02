<script lang="ts">
	import Viewport2D from '../../Viewport2D.svelte';
	import ViewportHandle from '../../ViewportHandle.svelte';
	let narrow = $state(false);
	let native = $state(false);
	let edits = $state(0);
</script>

<button onclick={() => (narrow = !narrow)}>Resize view</button>
<button onclick={() => (native = !native)}>Toggle native</button>
<output aria-label="Domain edit starts">{edits}</output>
<main style:width={narrow ? '32rem' : '50rem'}>
	<Viewport2D
		aspect={2}
		identity="fixture"
		{native}
		selection={{ x: 0.15, y: 0.2, width: 0.2, height: 0.25 }}
		label="Test viewport">
		<ViewportHandle x={0.35} y={0.4} label="Test handle" onpointerdown={() => (edits += 1)} />
		<div class="selection" aria-label="Test selection"></div>
	</Viewport2D>
</main>

<style>
	:global(html) {
		font-size: 16px;
		background: #333;
	}
	main {
		height: 32rem;
		margin: 1rem;
		background: #252525;
	}
	.selection {
		position: absolute;
		left: 15%;
		top: 20%;
		width: 20%;
		height: 25%;
		pointer-events: none;
		outline: 0.0625rem solid #f90;
	}
</style>
