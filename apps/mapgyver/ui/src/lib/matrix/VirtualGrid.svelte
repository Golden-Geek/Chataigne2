<script lang="ts">
	import { IconButton } from 'golden_ui';
	import { CanvasToolbar, canvasNavigationCommand } from 'golden_graph_ui';
	import SourceDropZone from '../media/SourceDropZone.svelte';
	import playIcon from 'golden_ui/style/icons/play.svg';
	import addIcon from 'golden_ui/style/icons/node/add.svg';
	import groupIcon from 'golden_ui/style/icons/node/folder.svg';
	import deleteIcon from 'golden_ui/style/icons/delete.svg';
	import { onMount, untrack } from 'svelte';
	import type { MatrixViewportSnapshot } from '../media/generated/MatrixViewportSnapshot';
	import type { MatrixViewport } from '../media/generated/MatrixViewport';
	import type { MatrixViewRow } from '../media/generated/MatrixViewRow';
	import type { ColumnSnapshot } from '../media/generated/ColumnSnapshot';
	import type { MatrixEdit } from '../media/generated/MatrixEdit';
	import type { MatrixState } from './authoring';
	import { gridSize, projectGrid, selectionScroll } from './viewport';
	import { sameSelection } from './selection';
	import type { MatrixRenderStatus } from '../preview/generated/MatrixRenderStatus';
	import GridLabel from './GridLabel.svelte';
	import CellPreviews from './CellPreviews.svelte';
	import { rowDropTarget, type GridDrag } from './grid-drag';
	let {
		snapshot,
		selected,
		observation,
		disabled,
		viewport,
		select,
		selectRow,
		selectColumn,
		launch,
		trigger,
		drop,
		edit,
		menu
	}: {
		snapshot: MatrixViewportSnapshot;
		selected: MatrixState['selected'];
		observation: MatrixRenderStatus | null;
		disabled: boolean;
		viewport(value: MatrixViewport): void;
		select(row: MatrixViewRow, column: ColumnSnapshot): void;
		selectRow(row: MatrixViewRow): void;
		selectColumn(column: ColumnSnapshot): void;
		launch(column: string): void;
		trigger(row: MatrixViewRow, column: ColumnSnapshot): void;
		drop(row: MatrixViewRow, column: ColumnSnapshot, source: string, copy: boolean): void;
		edit(value: MatrixEdit): Promise<unknown>;
		menu(
			event: MouseEvent,
			row?: MatrixViewRow,
			column?: ColumnSnapshot,
			creationOnly?: boolean
		): void;
	} = $props();
	const layerRequests = $derived(
		new Map(snapshot.playback.layers.map((request) => [request.layer, request] as const))
	);
	const submittedLayers = $derived(
		new Map((observation?.submitted?.layers ?? []).map((entry) => [entry.layer, entry] as const))
	);
	const cells = $derived(
		new Map(
			snapshot.cells.map((value) => [`${value.cell.layer}:${value.cell.definition.column}`, value])
		)
	);
	let element: HTMLDivElement;
	let dragging = $state<GridDrag | null>(null);
	let highlighted: HTMLElement | null = null;
	let dragIdentity = '';
	function clearDropHint() {
		highlighted?.removeAttribute('data-matrix-drop-position');
		highlighted = null;
	}
	function endDrag() {
		clearDropHint();
		dragging = null;
	}
	$effect(() => {
		const identity = JSON.stringify([snapshot.project, snapshot.media]);
		if (identity !== dragIdentity) {
			dragIdentity = identity;
			endDrag();
		}
	});
	function startDrag(event: DragEvent, value: GridDrag) {
		if (disabled || !event.dataTransfer || event.target instanceof HTMLInputElement) {
			event.preventDefault();
			return;
		}
		event.stopPropagation();
		dragging = value;
		event.dataTransfer.effectAllowed = 'move';
		event.dataTransfer.setData('application/x-mapgyver-matrix-drag', JSON.stringify(value));
	}
	function dropAt(
		event: DragEvent
	): { edit: MatrixEdit; element: HTMLElement; position: string } | null {
		if (!element || !dragging || disabled || !(event.target instanceof Element)) return null;
		const header = event.target.closest<HTMLElement>('[data-matrix-drop]');
		if (dragging.kind === 'Column') {
			if (header?.dataset.matrixDrop !== 'Column') return null;
			const target = header.dataset.columnId;
			if (!target || target === dragging.column) return null;
			const rect = header.getBoundingClientRect();
			const before = event.clientX < rect.left + rect.width / 2;
			return {
				edit: { kind: 'DropColumn', column: dragging.column, target, before },
				element: header,
				position: before ? 'left' : 'right'
			};
		}
		if (header?.dataset.matrixDrop === 'Item') {
			const row = snapshot.rows.find((row) => row.item.id.id === header.dataset.rowId);
			if (!row || row.item.id.id === dragging.item.id) return null;
			const rect = header.getBoundingClientRect();
			const target = rowDropTarget(row, (event.clientY - rect.top) / rect.height);
			return {
				edit: { kind: 'DropItem', item: dragging.item, target },
				element: header,
				position: target.kind.toLowerCase()
			};
		}
		if (
			header?.dataset.matrixDrop === 'Root' ||
			event.target === element ||
			event.target.classList.contains('canvas')
		)
			return {
				edit: { kind: 'DropItem', item: dragging.item, target: { kind: 'Root' } },
				element: header ?? element,
				position: 'inside'
			};
		return null;
	}
	let metrics = $state({ top: 0, left: 0, width: 0, height: 0, rem: 1 });
	const projection = $derived(projectGrid(metrics, snapshot.row_total, snapshot.column_total));
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
		const command = canvasNavigationCommand(event);
		if (command === 'home' || command === 'frame') {
			event.preventDefault();
			event.stopPropagation();
			if (command === 'home') home();
			else frameSelection();
			return;
		}
		const delta: Record<string, ScrollToOptions> = {
			ArrowDown: { top: gridSize.row * metrics.rem },
			ArrowUp: { top: -gridSize.row * metrics.rem },
			ArrowRight: { left: gridSize.column * metrics.rem },
			ArrowLeft: { left: -gridSize.column * metrics.rem },
			PageDown: { top: element.clientHeight },
			PageUp: { top: -element.clientHeight }
		};
		if (delta[event.key]) {
			event.preventDefault();
			element.scrollBy(delta[event.key]);
		}
	}
	const frameTarget = $derived(
		snapshot.selected && sameSelection(snapshot.selected.selection, selected)
			? snapshot.selected
			: null
	);
	const canFrame = $derived(frameTarget?.row_index != null || frameTarget?.column_index != null);
	const scrollBehavior = () =>
		window.matchMedia('(prefers-reduced-motion: reduce)').matches
			? ('auto' as const)
			: ('smooth' as const);
	function home() {
		element?.scrollTo({ top: 0, left: 0, behavior: scrollBehavior() });
	}
	function frameSelection() {
		if (!element || !frameTarget) return;
		measure();
		element.scrollTo({
			...selectionScroll(
				metrics,
				snapshot.row_total,
				snapshot.column_total,
				frameTarget.row_index,
				frameTarget.column_index
			),
			behavior: scrollBehavior()
		});
	}
	function context(event: MouseEvent) {
		event.preventDefault();
		event.stopPropagation();
		if (disabled) return;
		const target = event.target instanceof Element ? event.target : null;
		const row = snapshot.rows.find(
			(row) => row.item.id.id === target?.closest('[data-row-id]')?.getAttribute('data-row-id')
		);
		const column = snapshot.columns.find(
			(column) => column.id === target?.closest('[data-column-id]')?.getAttribute('data-column-id')
		);
		menu(event, row, column);
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

<svelte:window ondragend={endDrag} />

{#snippet matrixActions()}
	<IconButton
		icon={addIcon}
		label="Add layer"
		{disabled}
		onclick={() => edit({ kind: 'AddLayers', parent: null, count: 1 })} />
	<IconButton
		icon={groupIcon}
		label="Add group"
		{disabled}
		onclick={() => edit({ kind: 'AddGroup', parent: null, label: null })} />
	<IconButton
		icon={addIcon}
		label="Add column"
		{disabled}
		onclick={() => edit({ kind: 'AddColumns', count: 1 })} />
	<IconButton
		icon={deleteIcon}
		label="Unload Matrix"
		disabled={disabled || (!snapshot.playback.desired_column && !snapshot.playback.layers.length)}
		onclick={() => edit({ kind: 'ClearPlayback' })} />
{/snippet}

<div class="matrix-viewport-shell">
	<CanvasToolbar frame={frameSelection} {home} {canFrame} actions={matrixActions} />

	<div
		class="viewport"
		data-local-context-menu
		bind:this={element}
		onscroll={measure}
		onkeydown={navigate}
		oncontextmenu={context}
		ondragover={(event) => {
			const target = dropAt(event);
			clearDropHint();
			if (!target) return;
			event.preventDefault();
			event.stopPropagation();
			if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
			highlighted = target.element;
			highlighted.setAttribute('data-matrix-drop-position', target.position);
		}}
		ondragleave={(event) => {
			if (!(event.relatedTarget instanceof Node) || !element?.contains(event.relatedTarget))
				clearDropHint();
		}}
		ondrop={(event) => {
			const target = dropAt(event);
			endDrag();
			if (!target) return;
			event.preventDefault();
			event.stopPropagation();
			void edit(target.edit);
		}}
		role="grid"
		aria-label="Scrollable Matrix grid"
		aria-rowcount={snapshot.row_total + 1}
		aria-colcount={snapshot.column_total + 1}
		tabindex="0"
		style:--row={`${gridSize.row}rem`}
		style:--column={`${gridSize.column}rem`}
		style:--header={`${gridSize.header}rem`}
		style:--preview-label-height={`${gridSize.previewLabel}rem`}
		style:--label={`${gridSize.label}rem`}>
		<CellPreviews {snapshot} {metrics} />
		<div
			class="canvas"
			style:height={`${projection.canvasHeight}rem`}
			style:width={`${projection.canvasWidth}rem`}>
			<div
				class="corner"
				data-matrix-drop="Root"
				title="Drop a row here to move it to Matrix root"
				role="toolbar"
				aria-label="Matrix actions"
				style:top={`${projection.top}rem`}
				style:left={`${projection.left}rem`}>
			</div>
			<div role="row" aria-rowindex="1">
				{#each snapshot.columns as column, index (column.id)}
					<div
						class="heading"
						data-matrix-drop="Column"
						draggable={!disabled}
						ondragstart={(event) => startDrag(event, { kind: 'Column', column: column.id })}
						role="columnheader"
						tabindex="0"
						data-column-id={column.id}
						aria-colindex={snapshot.column_start + index + 2}
						class:requested={snapshot.playback.desired_column === column.id}
						class:installed={observation?.installed?.columns.includes(column.id) === true}
						class:submitted={observation?.submitted?.columns.includes(column.id) === true}
						style:top={`${projection.top}rem`}
						style:left={`${projection.columnOrigin + (snapshot.column_start + index) * gridSize.column}rem`}>
						<GridLabel
							label={column.label}
							selected={selected?.column === column.id}
							{disabled}
							select={() => selectColumn(column)}
							rename={async (label) =>
								Boolean(await edit({ kind: 'RenameColumn', column: column.id, label }))} />
						<div class="column-state">
							<IconButton
								{disabled}
								icon={playIcon}
								label={`Launch ${column.label}`}
								onclick={() => launch(column.id)} />
							{#if snapshot.playback.desired_column === column.id}
								<span
									class="request-marker"
									role="img"
									aria-label="Requested column"
									title="Requested column"></span>
							{/if}
							{#if observation?.submitted?.columns.includes(column.id)}
								<span
									class="submit-marker"
									role="img"
									aria-label="Renderer submitted this mix"
									title="Renderer submitted this mix"></span>
							{/if}
						</div>
					</div>
				{/each}
			</div>
			{#each snapshot.rows as row, index (row.item.id.id)}
				<div
					class="row"
					class:group={row.item.id.kind === 'Group'}
					role="row"
					data-row-id={row.item.id.id}
					aria-rowindex={snapshot.row_start + index + 2}
					style:top={`${projection.rowOrigin + (snapshot.row_start + index) * gridSize.row}rem`}>
					<div
						class="row-label"
						data-matrix-drop="Item"
						data-row-id={row.item.id.id}
						draggable={!disabled}
						ondragstart={(event) => startDrag(event, { kind: 'Item', item: row.item.id })}
						role="rowheader"
						tabindex="0"
						style:left={`${projection.left}rem`}
						style:padding-left={`${0.5 + row.depth * 0.8}rem`}
						title={row.item.label}>
						{#if row.item.id.kind === 'Group'}<img src={groupIcon} alt="" /><small>Group</small
							>{/if}
						<GridLabel
							label={row.item.label}
							selected={selected?.item?.id === row.item.id.id}
							{disabled}
							select={() => selectRow(row)}
							rename={async (label) =>
								Boolean(await edit({ kind: 'RenameItem', item: row.item.id, label }))} />
					</div>
					{#each snapshot.columns as column, columnIndex (column.id)}
						{@const cell = cells.get(`${row.item.id.id}:${column.id}`)}
						<SourceDropZone
							disabled={disabled || row.item.id.kind !== 'Layer'}
							addLabel="Set media"
							replaceOnly={cell?.cell.media ? column.id : null}
							data-column-id={column.id}
							style={`position:absolute;top:0;left:${projection.columnOrigin + (snapshot.column_start + columnIndex) * gridSize.column}rem;width:var(--column);height:var(--row)`}
							onsource={(source, _target, event) => drop(row, column, source, event.altKey)}>
							<button
								class="cell"
								class:has-preview={cell?.source_preview != null && !cell?.source_missing}
								class:group-cell={row.item.id.kind === 'Group'}
								role="gridcell"
								{disabled}
								aria-selected={selected?.item?.id === row.item.id.id &&
									selected?.column === column.id}
								aria-colindex={snapshot.column_start + columnIndex + 2}
								class:selected={selected?.item?.id === row.item.id.id &&
									selected?.column === column.id}
								onclick={() => select(row, column)}
								ondblclick={() => {
									if (row.item.id.kind === 'Layer') trigger(row, column);
								}}
								data-requested={(layerRequests.has(row.item.id.id)
									? layerRequests.get(row.item.id.id)?.column
									: snapshot.playback.desired_column) === column.id}
								data-submitted={(
									submittedLayers.get(row.item.id.id)?.columns ??
									observation?.submitted?.columns ??
									[]
								).includes(column.id)}
								title={`${row.item.label} / ${column.label}`}>
								<span class="source-label" title={cell?.source_label ?? undefined}
									>{row.item.id.kind === 'Group'
										? ''
										: cell?.source_missing
											? 'Missing source'
											: (cell?.source_label ?? '')}</span>
							</button>
							{#if row.item.id.kind === 'Layer'}
								{#if cell?.cell.media}
									<span class="remove"
										><IconButton
											icon={deleteIcon}
											label={`Remove media from ${row.item.label} / ${column.label}`}
											{disabled}
											onclick={() =>
												edit({
													kind: 'SetCells',
													assignments: [{ layer: row.item.id.id, column: column.id, source: null }]
												})} /></span>
								{:else}
									<span class="add"
										><IconButton
											icon={addIcon}
											label={`Add Media to ${row.item.label} / ${column.label}`}
											{disabled}
											onclick={(event) => {
												select(row, column);
												menu(event, row, column, true);
											}} /></span>
								{/if}
							{/if}
						</SourceDropZone>
					{/each}
				</div>
			{/each}
		</div>
	</div>
</div>

<style>
	.matrix-viewport-shell {
		position: relative;
		display: flex;
		flex-direction: column;
		flex: 1;
		min-height: 0;
	}
	.viewport {
		flex: 1;
		margin-block-start: 3rem;
		height: 100%;
		min-height: 0;
		min-width: 0;
		overflow: auto;
		position: relative;
		background: transparent;
		color: var(--gc-color-text);
	}
	.canvas {
		position: relative;
		min-width: 100%;
		min-height: 100%;
	}
	.corner,
	.heading {
		position: absolute;
		height: var(--header);
		box-sizing: border-box;
		background: var(--gc-color-header);
		z-index: 3;
		padding: 0.25rem;
		border: 0.05rem solid var(--gc-color-border);
	}
	.corner {
		width: var(--label);
		z-index: 5;
		display: flex;
		align-items: center;
		gap: 0.25rem;
	}
	.heading {
		width: var(--column);
		display: flex;
		align-items: center;
		gap: 0.2rem;
		flex-wrap: wrap;
	}
	.column-state {
		display: flex;
		align-items: center;
		gap: 0.4rem;
	}
	.request-marker,
	.submit-marker {
		width: 0.35rem;
		height: 0.35rem;
		border: 0.08rem solid var(--gc-color-focus);
		border-radius: 50%;
		flex: none;
	}
	.submit-marker {
		background: var(--gc-color-node-enabled);
		border-color: var(--gc-color-node-enabled);
	}
	.heading :global(button) {
		max-width: 100%;
	}
	.requested {
		background: color-mix(in srgb, var(--gc-color-selection) 20%, var(--gc-color-header));
	}
	.installed {
		box-shadow: inset 0 -0.15rem var(--gc-color-node-inactive);
	}
	.submitted {
		box-shadow: inset 0 -0.2rem var(--gc-color-node-enabled);
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
		background: var(--gc-color-header);
		padding: 0.5rem;
		overflow: hidden;
		display: flex;
		flex-direction: column;
		justify-content: center;
		gap: 0.3rem;
		border: 0.05rem solid var(--gc-color-border);
	}
	.row-label img {
		width: 1.2em;
		height: 1.2em;
	}
	.group .row-label {
		border-left: 0.2rem solid var(--gc-color-accent);
	}
	.cell {
		position: absolute;
		inset: 0;
		box-sizing: border-box;
		height: var(--row);
		width: var(--column);
		overflow: hidden;
		overflow-wrap: anywhere;
		white-space: normal;
		background: var(--gc-color-panel);
		color: inherit;
		border: 0.05rem solid var(--gc-color-border);
		padding: 1.25rem 0.5rem;
	}
	.group-cell {
		background: repeating-linear-gradient(
			135deg,
			var(--gc-color-header),
			var(--gc-color-header) 0.5rem,
			var(--gc-color-panel) 0.5rem,
			var(--gc-color-panel) 1rem
		);
		opacity: 0.6;
	}
	.cell[data-requested='true']::before,
	.cell[data-submitted='true']::after {
		content: '';
		position: absolute;
		top: 0.35rem;
		width: 0.35rem;
		height: 0.35rem;
		border: 0.08rem solid var(--gc-color-focus);
		border-radius: 50%;
	}
	.cell[data-requested='true']::before {
		left: 0.35rem;
	}
	.cell[data-submitted='true']::after {
		left: 1rem;
		background: var(--gc-color-node-enabled);
		border-color: var(--gc-color-node-enabled);
	}
	.cell.selected {
		outline: 0.15rem solid var(--gc-color-focus);
		outline-offset: -0.2rem;
	}
	.cell.has-preview {
		background: transparent;
	}
	.source-label {
		position: absolute;
		inset: auto 0.15rem 0.15rem;
		max-height: var(--preview-label-height);
		overflow: hidden;
		font-size: 0.8em;
		line-height: calc(var(--preview-label-height) / 2);
		display: -webkit-box;
		-webkit-box-orient: vertical;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		background: var(--gc-color-panel);
	}
	.add,
	.remove {
		position: absolute;
		z-index: 1;
	}
	.add {
		left: 50%;
		top: 50%;
		transform: translate(-50%, -50%);
	}
	.remove {
		right: 0.25rem;
		top: 0.25rem;
	}
	small {
		font-size: 0.7em;
	}
	:global([data-matrix-drop-position='before']) {
		box-shadow: inset 0 0.2rem var(--gc-color-focus);
	}
	:global([data-matrix-drop-position='after']) {
		box-shadow: inset 0 -0.2rem var(--gc-color-focus);
	}
	:global([data-matrix-drop-position='left']) {
		box-shadow: inset 0.2rem 0 var(--gc-color-focus);
	}
	:global([data-matrix-drop-position='right']) {
		box-shadow: inset -0.2rem 0 var(--gc-color-focus);
	}
	:global([data-matrix-drop-position='inside']) {
		outline: 0.15rem solid var(--gc-color-focus);
		outline-offset: -0.2rem;
	}
</style>
