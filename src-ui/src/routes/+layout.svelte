<script lang="ts">
	import favicon from "$lib/assets/favicon.svg";

	import "$lib/golden-core.css";

	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>


{@render children()}

