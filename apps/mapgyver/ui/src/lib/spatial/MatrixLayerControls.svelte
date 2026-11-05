<script lang="ts">
	import type { MatrixViewRow } from '../media/generated/MatrixViewRow';
	let {
		rows,
		canvas,
		selected,
		disabled,
		select
	}: {
		rows: MatrixViewRow[];
		canvas: { width: number; height: number };
		selected: string | null;
		disabled: boolean;
		select(uuid: string): void;
	} = $props();
</script>

<svg
	viewBox={`0 0 ${canvas.width} ${canvas.height}`}
	preserveAspectRatio="none"
	role="group"
	aria-label="Matrix Layer layout controls">
	{#each rows as row (row.item.id.id)}
		{#if row.item.layer}
			<rect
				data-layer-id={row.item.id.id}
				x={row.item.layer.position[0]}
				y={row.item.layer.position[1]}
				width={row.item.layer.size.width}
				height={row.item.layer.size.height}
				class:selected={selected === row.item.id.id}
				class:inactive={!row.item.settings.enabled}
				role="button"
				tabindex={disabled ? -1 : 0}
				aria-label={`Select ${row.item.label}`}
				aria-disabled={disabled}
				onclick={(event) => {
					event.stopPropagation();
					if (!disabled) select(row.item.id.id);
				}}
				onkeydown={(event) => {
					if (!disabled && (event.key === 'Enter' || event.key === ' ')) {
						event.preventDefault();
						select(row.item.id.id);
					}
				}} />
		{/if}
	{/each}
</svg>

<style>
	svg {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		overflow: visible;
	}
	rect {
		fill: transparent;
		stroke: var(--gc-color-border);
		stroke-width: 0.06rem;
		vector-effect: non-scaling-stroke;
		cursor: pointer;
	}
	rect.selected,
	rect:focus {
		stroke: var(--gc-color-focus);
		stroke-width: 0.12rem;
	}
	rect.inactive {
		stroke-dasharray: 0.3rem 0.2rem;
	}
</style>
