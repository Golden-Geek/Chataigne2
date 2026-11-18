<script lang="ts">
	import { lineWeight, type PlaybackLines } from './playback-mix';
	let { lines, time }: { lines: PlaybackLines; time(): number } = $props();
</script>

{#if lines.incoming.length}
	<span
		aria-hidden="true"
		data-matrix-weight="incoming"
		style:transform={`scaleX(${lineWeight(lines.incoming, time())})`}></span>
{/if}
{#if lines.outgoing.length}
	<span
		aria-hidden="true"
		data-matrix-weight="outgoing"
		style:transform={`scaleX(${lineWeight(lines.outgoing, time())})`}></span>
{/if}

<style>
	span {
		position: absolute;
		inset: auto 0 0;
		height: 0.18rem;
		border-radius: 0.09rem;
		background: var(--gc-color-node-enabled);
		pointer-events: none;
		transform-origin: left center;
	}
	span[data-matrix-weight='outgoing'] {
		transform-origin: right center;
	}
</style>
