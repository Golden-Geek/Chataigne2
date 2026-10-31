<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import type { MatrixViewportSnapshot } from '../media/generated/MatrixViewportSnapshot';
	import type { MatrixViewport } from '../media/generated/MatrixViewport';
	import type { MatrixViewRow } from '../media/generated/MatrixViewRow';
	import type { ColumnSnapshot } from '../media/generated/ColumnSnapshot';
	import type { MatrixState } from './authoring';
	import { gridSize, projectGrid } from './viewport';
	let {
		snapshot,
		selected,
		disabled,
		viewport,
		select,
		launch
	}: {
		snapshot: MatrixViewportSnapshot;
		selected: MatrixState['selected'];
		disabled: boolean;
		viewport(value: MatrixViewport): void;
		select(row: MatrixViewRow, column: ColumnSnapshot): void;
		launch(column: string): void;
	} = $props();
	let element: HTMLDivElement;
	let metrics = $state({ top: 0, left: 0, width: 0, height: 0, rem: 1 });
	const projection = $derived(projectGrid(metrics, snapshot.row_total, snapshot.column_total));
	const cells = $derived(
		new Map(
			snapshot.cells.map((value) => [`${value.cell.layer}:${value.cell.definition.column}`, value])
		)
	);
	function measure() {
		if (!element) return;
		metrics = {
			top: element.scrollTop,
			left: element.scrollLeft,
			width: element.clientWidth,
			height: element.clientHeight,
			rem: Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 1
		};
	}
	function navigate(event: KeyboardEvent) {
		if (event.target !== element) return;
		const row = gridSize.row * metrics.rem,
			column = gridSize.column * metrics.rem;
		const delta: Record<string, ScrollToOptions> = {
			ArrowDown: { top: row },
			ArrowUp: { top: -row },
			ArrowRight: { left: column },
			ArrowLeft: { left: -column },
			PageDown: { top: element.clientHeight },
			PageUp: { top: -element.clientHeight }
		};
		if (delta[event.key]) {
			event.preventDefault();
			element.scrollBy(delta[event.key]);
		}
	}
	$effect(() => {
		const wanted = projection.viewport;
		untrack(() => viewport(wanted));
	});
	onMount(() => {
		const resize = new ResizeObserver(measure);
		resize.observe(element);
		measure();
		return () => resize.disconnect();
	});
</script>

<div
	class="viewport"
	bind:this={element}
	onscroll={measure}
	onkeydown={navigate}
	role="grid"
	aria-label="Scrollable Matrix grid"
	aria-rowcount={snapshot.row_total + 1}
	aria-colcount={snapshot.column_total + 1}
	tabindex="0"
	style:--row={`${gridSize.row}rem`}
	style:--column={`${gridSize.column}rem`}
	style:--label={`${gridSize.label}rem`}>
	<div
		class="canvas"
		style:height={`${projection.canvasHeight}rem`}
		style:width={`${projection.canvasWidth}rem`}>
		<div class="corner" style:top={`${projection.top}rem`} style:left={`${projection.left}rem`}>
			Layers / columns
		</div>
		<div role="row" aria-rowindex="1">
			{#each snapshot.columns as column, index (column.id)}
				<div
					class="heading"
					role="columnheader"
					aria-colindex={snapshot.column_start + index + 2}
					class:requested={snapshot.playback.desired_column === column.id}
					style:top={`${projection.top}rem`}
					style:left={`${projection.columnOrigin + (snapshot.column_start + index) * gridSize.column}rem`}>
					<button {disabled} onclick={() => launch(column.id)} title={`Launch ${column.label}`}
						>{column.label}</button>
					{#if snapshot.playback.desired_column === column.id}<small>Requested</small>{/if}
				</div>
			{/each}
		</div>
		{#each snapshot.rows as row, index (row.item.id.id)}
			<div
				class="row"
				role="row"
				aria-rowindex={snapshot.row_start + index + 2}
				style:top={`${projection.rowOrigin + (snapshot.row_start + index) * gridSize.row}rem`}>
				<div
					class="row-label"
					role="rowheader"
					style:left={`${projection.left}rem`}
					style:padding-left={`${0.5 + row.depth * 0.8}rem`}
					title={row.item.label}>
					{row.item.id.kind === 'Group' ? '▾ ' : ''}{row.item.label}
				</div>
				{#each snapshot.columns as column, columnIndex (column.id)}
					{@const cell = cells.get(`${row.item.id.id}:${column.id}`)}
					<button
						class="cell"
						role="gridcell"
						{disabled}
						aria-selected={selected?.item.id === row.item.id.id && selected?.column === column.id}
						aria-colindex={snapshot.column_start + columnIndex + 2}
						class:selected={selected?.item.id === row.item.id.id && selected?.column === column.id}
						style:left={`${projection.columnOrigin + (snapshot.column_start + columnIndex) * gridSize.column}rem`}
						onclick={() => select(row, column)}
						title={`${row.item.label} / ${column.label}`}>
						{row.item.id.kind === 'Group'
							? 'Group'
							: cell?.source_missing
								? 'Missing source'
								: (cell?.source_label ?? 'Empty')}
					</button>
				{/each}
			</div>
		{/each}
	</div>
</div>

<style>
	.viewport {
		flex: 1;
		min-height: 12rem;
		min-width: 0;
		overflow: auto;
		position: relative;
		background: var(--mapgyver-panel-background, #191b20);
	}
	.canvas {
		position: relative;
		min-width: 100%;
		min-height: 100%;
	}
	.corner,
	.heading {
		position: absolute;
		height: 3rem;
		box-sizing: border-box;
		background: #292d36;
		z-index: 3;
		padding: 0.25rem;
	}
	.corner {
		width: var(--label);
		z-index: 5;
	}
	.heading {
		width: var(--column);
		display: flex;
		flex-direction: column;
	}
	.heading button {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.requested {
		background: #364c69;
	}
	.row {
		position: absolute;
		height: var(--row);
		width: 100%;
	}
	.row-label {
		position: absolute;
		z-index: 2;
		box-sizing: border-box;
		width: var(--label);
		height: var(--row);
		background: #292d36;
		padding: 0.5rem;
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}
	.cell {
		position: absolute;
		box-sizing: border-box;
		height: var(--row);
		width: var(--column);
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
		background: #20242c;
		color: inherit;
		border: 0.05rem solid #414754;
	}
	.cell.selected {
		outline: 0.15rem solid #93b9eb;
		outline-offset: -0.2rem;
	}
	small {
		font-size: 0.7em;
	}
</style>
