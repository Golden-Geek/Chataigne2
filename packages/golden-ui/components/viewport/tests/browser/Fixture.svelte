<script lang="ts">
	import Viewport2D from '../../Viewport2D.svelte';
	import ViewportHandle from '../../ViewportHandle.svelte';
	let narrow = $state(false);
	let native = $state(false);
	let edits = $state(0);
	let backgrounds = $state(0);
	let clears = $state(0);
	let document = $state({
		id: 'fixture',
		aspect: 2,
		revision: 0,
		selection: { x: 0.15, y: 0.2, width: 0.2, height: 0.25 }
	});
	function replaceSnapshot() {
		document = { ...document, revision: document.revision + 1 };
	}
</script>

<button onclick={() => (narrow = !narrow)}>Resize view</button>
<button onclick={() => (native = !native)}>Toggle native</button>
<button onclick={replaceSnapshot}>Update geometry snapshot</button>
<button
	onclick={() => {
		document = {
			...document,
			selection: { ...document.selection, x: document.selection.x + 0.05 }
		};
	}}>Change selection</button>
<button
	onclick={() => {
		document = { ...document, id: `${document.id}-next` };
	}}>Change document</button>
<output aria-label="Domain edit starts">{edits}</output>
<output aria-label="Background clicks">{backgrounds}</output>
<output aria-label="Escape clears">{clears}</output>
<main style:width={narrow ? '32rem' : '50rem'}>
	<Viewport2D
		aspect={document.aspect}
		identity={document.id}
		onselectionclear={(event) => {
			if (event.type === 'pointerdown') backgrounds++;
			else clears++;
		}}
		{native}
		selection={document.selection}
		label="Test viewport">
		<ViewportHandle
			x={0.35}
			y={0.4}
			label="Test handle"
			onkeydown={(event) => {
				if (event.key === 'Escape') event.preventDefault();
			}}
			onpointerdown={() => {
				edits += 1;
				replaceSnapshot();
				queueMicrotask(replaceSnapshot);
			}} />
		<div
			class="selection"
			aria-label="Test selection"
			style:left={`${document.selection.x * 100}%`}>
		</div>
		<input aria-label="Viewport text" style:position="absolute" style:top="0.2rem" />
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
