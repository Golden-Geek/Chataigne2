<script lang="ts">
	import { onMount } from 'svelte';
	import { NativePreview, createDesktopPreviewTransport, createPreviewSession } from '../../index';
	const session = createPreviewSession(createDesktopPreviewTransport());
	let source = $state('first-screen');
	let inspector = $state(false);
	onMount(() => {
		void session.connect();
		return () => {
			void session.dispose();
		};
	});
</script>

<button onclick={() => (source = source === 'first-screen' ? 'second-screen' : 'first-screen')}>
	Change screen
</button>
<button onclick={() => (inspector = !inspector)}>Toggle Inspector preview</button>
<main>
	<section data-continuity-screen>
		<NativePreview {session} {source} label="Screen preview" />
	</section>
	<aside>
		{#if inspector}<NativePreview {session} source="inspector" label="Inspector preview" />{/if}
	</aside>
</main>

<style>
	main {
		display: grid;
		grid-template-columns: 2fr 1fr;
		gap: 1rem;
		width: 45rem;
		max-width: 100%;
	}
	section,
	aside {
		height: 15rem;
	}
</style>
