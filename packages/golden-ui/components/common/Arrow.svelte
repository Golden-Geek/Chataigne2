<script lang="ts">
	let {
		direction = 'right',
		color = 'rgba(255, 255, 255, 0.5)',
		size = '1rem'
	}: {
		direction: 'left' | 'right' | 'up' | 'down';
		color?: string;
		size?: string;
	} = $props();
</script>

<svg
	class="arrow {direction}"
	viewBox="0 0 24 24"
	fill="none"
	xmlns="http://www.w3.org/2000/svg"
	style="--arrow-size: {size}">
	<path
		d="M 8 0 M 15 12 L 9 17 M 15 12 L 9 7"
		stroke={color}
		stroke-width="3.5"
		stroke-linecap="round"
		stroke-linejoin="round" />
</svg>

<style>
	.arrow {
		vertical-align: middle;
		width: var(--arrow-size);
		height: var(--arrow-size);
		background-size: contain;
		transition:
			opacity 0.2s ease,
			transform 0.1s ease;
		cursor: pointer;
		opacity: 0.7;
		margin-right: 0rem;
	}

	.arrow:hover {
		opacity: 1;
	}

	.arrow.right {
		transform: rotate(0deg);
	}
	.arrow.down {
		transform: rotate(90deg);
	}
	.arrow.left {
		transform: rotate(180deg);
	}
	.arrow.up {
		transform: rotate(270deg);
	}
</style>
