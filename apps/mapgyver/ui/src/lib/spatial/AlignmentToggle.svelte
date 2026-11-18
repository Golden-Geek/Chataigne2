<script lang="ts">
	import { IconButton } from 'golden_ui';
	import magnet from '../screens/icons/magnet.svg';
	let { enabled = $bindable(false), disabled = false }: { enabled?: boolean; disabled?: boolean } =
		$props();
</script>

<span class:off={!enabled}>
	<IconButton
		icon={magnet}
		label="Align geometry"
		title="Snap edges and centers to visible objects and canvas bounds"
		aria-pressed={enabled}
		{disabled}
		onclick={() => {
			enabled = !enabled;
		}} />
</span>

<style>
	span {
		display: inline-flex;
	}
	.off :global(img) {
		filter: grayscale(1);
		opacity: 0.45;
	}
</style>
