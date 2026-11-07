<script lang="ts">
	import { IconButton, registerCommandHandler } from 'golden_ui';
	import SourceDropZone from '../media/SourceDropZone.svelte';
	import playIcon from 'golden_ui/style/icons/play.svg';
	import addIcon from 'golden_ui/style/icons/node/add.svg';
	import groupIcon from 'golden_ui/style/icons/node/folder.svg';
	import deleteIcon from 'golden_ui/style/icons/panels/close.svg';
	import emptyIcon from '../icons/empty-cell.svg';
	import { onMount, untrack } from 'svelte';
	import type { MatrixViewportSnapshot } from '../media/generated/MatrixViewportSnapshot';
	import type { MatrixViewport } from '../media/generated/MatrixViewport';
	import type { MatrixViewRow } from '../media/generated/MatrixViewRow';
	import type { ColumnSnapshot } from '../media/generated/ColumnSnapshot';
	import type { MatrixEdit } from '../media/generated/MatrixEdit';
	import type { MatrixState } from './authoring';
	import { gridSize, projectGrid } from './viewport';
	import GridCreationHeaders from './GridCreationHeaders.svelte';
	import type { MatrixRenderStatus } from '../preview/generated/MatrixRenderStatus';
	import type { ApplicationRuntimeSnapshot } from '../media/generated/ApplicationRuntimeSnapshot';
	import GridLabel from './GridLabel.svelte';
	import RowControls from './RowControls.svelte';
	import ColumnGroupHeaders from './ColumnGroupHeaders.svelte';
	import type { ColumnGroupSnapshot } from '../media/generated/ColumnGroupSnapshot';
	import CellPreviews from './CellPreviews.svelte';
	import StopAllControl from './StopAllControl.svelte';
	import ParameterTrigger from './ParameterTrigger.svelte';
	import { rowDropTarget, type GridDrag } from './grid-drag';
	import { CELL_DRAG_TYPE, readCellDrag, type CellDrag } from './cell-transfer';
	import { sameSelection } from './selection';
	let {
		snapshot,
		selected,
		observation,
		applicationRuntime,
		disabled,
		viewport,
		scroll = { top: 0, left: 0 },
		onscrollposition = () => {},
		select,
		selectRow,
		selectColumn,
		selectGroup,
		selectedGroup,
		trigger,
		drop,
		cellSource,
		cellDrop,
		cellCopy,
		cellPaste,
		edit,
		menu,
		createMenu
	}: {
		snapshot: MatrixViewportSnapshot;
		selected: MatrixState['selected'];
		observation: MatrixRenderStatus | null;
		applicationRuntime: ApplicationRuntimeSnapshot | null;
		disabled: boolean;
		viewport(value: MatrixViewport): void;
		scroll?: { top: number; left: number };
		onscrollposition?(value: { top: number; left: number }, media: string): void;
		select(row: MatrixViewRow, column: ColumnSnapshot): void;
		selectRow(row: MatrixViewRow): void;
		selectColumn(column: ColumnSnapshot): void;
		selectGroup(group: ColumnGroupSnapshot): void;
		selectedGroup: string | null;
		trigger(row: MatrixViewRow, column: ColumnSnapshot): void;
		drop(row: MatrixViewRow, column: ColumnSnapshot, source: string, copy: boolean): void;
		cellSource(cell: string): CellDrag | null;
		cellDrop(source: CellDrag, layer: string, column: string, event: DragEvent): void;
		cellCopy(cell: string, cut?: boolean): boolean;
		cellPaste(layer: string, column: string, reference?: boolean): Promise<boolean>;
		edit(value: MatrixEdit): Promise<unknown>;
		createMenu(event: MouseEvent, axis: 'Column' | 'Layer'): void;
		menu(
			event: MouseEvent,
			row?: MatrixViewRow,
			column?: ColumnSnapshot,
			creationOnly?: boolean,
			group?: ColumnGroupSnapshot
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
	let dragFromControl = false;
	function clearDropHint() {
		highlighted?.removeAttribute('data-matrix-drop-position');
		highlighted = null;
	}
	function endDrag() {
		clearDropHint();
		dragging = null;
		dragFromControl = false;
	}
	$effect(() => {
		const identity = JSON.stringify([snapshot.project, snapshot.media]);
		if (identity !== dragIdentity) {
			dragIdentity = identity;
			endDrag();
		}
	});
	function startDrag(event: DragEvent, value: GridDrag) {
		if (
			disabled ||
			dragFromControl ||
			!event.dataTransfer ||
			(event.target instanceof Element &&
				event.target.closest('input, select, textarea, [data-matrix-row-controls]'))
		) {
			event.preventDefault();
			return;
		}
		event.stopPropagation();
		dragging = value;
		event.dataTransfer.effectAllowed = 'move';
		event.dataTransfer.setData('application/x-mapgyver-matrix-drag', JSON.stringify(value));
	}
	function startCellDrag(event: DragEvent, cell: string) {
		const source = disabled ? null : cellSource(cell);
		if (!source || !event.dataTransfer) {
			event.preventDefault();
			return;
		}
		event.stopPropagation();
		event.dataTransfer.effectAllowed = 'all';
		event.dataTransfer.setData(CELL_DRAG_TYPE, JSON.stringify(source));
	}
	function cellTarget(event: DragEvent) {
		if (
			disabled ||
			!event.dataTransfer?.types.includes(CELL_DRAG_TYPE) ||
			!(event.target instanceof Element)
		)
			return null;
		const target = event.target.closest<HTMLElement>('[data-cell-drop]');
		return target && element.contains(target) ? target : null;
	}
	function selectHeader(event: MouseEvent | KeyboardEvent, select: () => void) {
		if (disabled) return;
		if (event instanceof KeyboardEvent) {
			if (event.target !== event.currentTarget || !['Enter', ' '].includes(event.key)) return;
			event.preventDefault();
			event.stopPropagation();
		} else if (
			event.target instanceof Element &&
			event.target.closest('button, input, select, textarea, [role="slider"]')
		)
			return;
		select();
	}
	function dropAt(
		event: DragEvent
	): { edit: MatrixEdit; element: HTMLElement; position: string } | null {
		if (!element || !dragging || disabled || !(event.target instanceof Element)) return null;
		const header = event.target.closest<HTMLElement>('[data-matrix-drop]');
		if (dragging.kind !== 'Item') {
			const item =
				dragging.kind === 'Column'
					? { kind: 'Column' as const, id: dragging.column }
					: { kind: 'Group' as const, id: dragging.group };
			if (header?.dataset.matrixDrop === 'ColumnGroup') {
				const group = header.dataset.columnGroupId;
				if (!group || group === item.id) return null;
				const rect = header.getBoundingClientRect();
				const fraction = (event.clientX - rect.left) / rect.width;
				const target =
					fraction >= 1 / 3 && fraction <= 2 / 3
						? { kind: 'Inside' as const, group }
						: {
								kind: fraction < 0.5 ? ('Before' as const) : ('After' as const),
								item: { kind: 'Group' as const, id: group }
							};
				return {
					edit: { kind: 'DropColumnItem', item, target },
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
					edit: { kind: 'DropColumnItem', item, target: { kind: 'Root' } },
					element: header ?? element,
					position: 'inside'
				};
			if (header?.dataset.matrixDrop !== 'Column') return null;
			const target = header.dataset.columnId;
			if (!target || target === item.id) return null;
			const rect = header.getBoundingClientRect();
			const before = event.clientX < rect.left + rect.width / 2;
			return {
				edit: {
					kind: 'DropColumnItem',
					item,
					target: { kind: before ? 'Before' : 'After', item: { kind: 'Column', id: target } }
				},
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
	const projection = $derived(
		projectGrid(metrics, snapshot.row_total, snapshot.axis_total, snapshot.axis_depth)
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
		onscrollposition(
			{ top: metrics.top / metrics.rem, left: metrics.left / metrics.rem },
			snapshot.media
		);
	}
	function transferKey(event: KeyboardEvent) {
		if (
			!disabled &&
			(event.ctrlKey || event.metaKey) &&
			event.target instanceof Element &&
			!event.target.closest('input,textarea,select,[contenteditable=true],[role=slider]')
		) {
			const selectedCell =
				snapshot.selected && sameSelection(snapshot.selected.selection, selected)
					? snapshot.selected.cell?.cell
					: selected?.item
						? cells.get(`${selected.item.id}:${selected.column}`)?.cell
						: null;
			const key = event.key.toLowerCase();
			if (
				(key === 'c' || key === 'x') &&
				!event.altKey &&
				selectedCell &&
				cellCopy(selectedCell.id, key === 'x')
			) {
				event.preventDefault();
				event.stopPropagation();
				return;
			}
			if (key === 'v' && selected?.item?.kind === 'Layer' && selected.column) {
				event.preventDefault();
				event.stopPropagation();
				void cellPaste(selected.item.id, selected.column, event.altKey);
				return;
			}
		}
	}
	function navigate(event: KeyboardEvent) {
		// Alt+Paste is a specialized Reference action; ordinary clipboard keys use Golden commands.
		if (event.altKey) transferKey(event);
		if (event.target !== element) return;
		const delta: Record<string, ScrollToOptions> = {
			ArrowDown: { top: projection.row * metrics.rem },
			ArrowUp: { top: -projection.row * metrics.rem },
			ArrowRight: { left: projection.column * metrics.rem },
			ArrowLeft: { left: -projection.column * metrics.rem },
			PageDown: { top: element.clientHeight },
			PageUp: { top: -element.clientHeight }
		};
		if (delta[event.key]) {
			event.preventDefault();
			element.scrollBy(delta[event.key]);
		}
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
		const group = snapshot.column_groups.find(
			(group) =>
				group.id === target?.closest('[data-column-group-id]')?.getAttribute('data-column-group-id')
		);
		menu(event, row, column, false, group);
	}
	$effect(() => {
		const wanted = projection.viewport;
		untrack(() => viewport(wanted));
	});
	onMount(() => {
		const commands = (['edit.copy', 'edit.cut', 'edit.paste'] as const).map((command) =>
			registerCommandHandler(
				command,
				({ event }) => {
					if (
						!event ||
						!(event.target instanceof Element) ||
						!element.contains(event.target) ||
						event.target.closest('input,textarea,select,[contenteditable=true],[role=slider]')
					)
						return false;
					// Claim this panel's clipboard command before generic subtree editing can run.
					event.stopPropagation();
					transferKey(event);
					return true;
				},
				{ priority: 200 }
			)
		);
		const rem = Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 1;
		element.scrollTop = scroll.top * rem;
		element.scrollLeft = scroll.left * rem;
		const resize = new ResizeObserver(measure);
		resize.observe(element);
		measure();
		return () => {
			resize.disconnect();
			for (const dispose of commands) dispose();
		};
	});
</script>

<svelte:window ondragend={endDrag} />

<div class="matrix-viewport-shell">
	<div class="viewport-container">
		<div
			class="viewport"
			data-local-context-menu
			bind:this={element}
			onscroll={measure}
			onkeydown={navigate}
			onpointerdown={(event) => {
				dragFromControl =
					event.target instanceof Element && !!event.target.closest('[data-matrix-row-controls]');
			}}
			oncontextmenu={context}
			ondragover={(event) => {
				const cell = cellTarget(event);
				if (cell) {
					clearDropHint();
					event.preventDefault();
					event.stopPropagation();
					if (event.dataTransfer) event.dataTransfer.dropEffect = event.altKey ? 'copy' : 'move';
					highlighted = cell;
					cell.setAttribute('data-matrix-drop-position', 'inside');
					return;
				}
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
				const cell = cellTarget(event);
				if (event.dataTransfer?.types.includes(CELL_DRAG_TYPE)) {
					const source = readCellDrag(event.dataTransfer.getData(CELL_DRAG_TYPE));
					endDrag();
					event.preventDefault();
					event.stopPropagation();
					if (source && cell?.dataset.cellLayer && cell.dataset.cellColumn)
						cellDrop(source, cell.dataset.cellLayer, cell.dataset.cellColumn, event);
					return;
				}
				const target = dropAt(event);
				endDrag();
				if (!target) return;
				event.preventDefault();
				event.stopPropagation();
				void edit(target.edit);
			}}
			role="grid"
			aria-label="Scrollable Matrix grid"
			aria-rowcount={snapshot.row_total + 2}
			aria-colcount={snapshot.axis_total + 2}
			tabindex="0"
			style:--row={`${projection.row}rem`}
			style:--column={`${projection.column}rem`}
			style:--header={`${gridSize.header}rem`}
			style:--preview-label-height={`${gridSize.previewLabel}rem`}
			style:--label={`${gridSize.label}rem`}>
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
					style:height={`${projection.header}rem`}
					style:left={`${projection.left}rem`}>
					<StopAllControl media={snapshot.media} {disabled} />
				</div>
				<GridCreationHeaders
					{projection}
					rows={snapshot.row_total}
					columns={snapshot.axis_total}
					{disabled}
					menu={createMenu} />
				<ColumnGroupHeaders
					groups={snapshot.column_groups}
					requested={snapshot.requested_column_groups}
					{observation}
					{applicationRuntime}
					start={snapshot.axis_start}
					end={snapshot.axis_start + projection.viewport.axis_count}
					depth={snapshot.axis_depth}
					height={projection.groupHeight}
					width={metrics.width / Math.max(metrics.rem, 1)}
					top={projection.top}
					left={projection.left}
					origin={projection.columnOrigin}
					columnSize={projection.column}
					selected={selectedGroup}
					{disabled}
					select={selectGroup}
					{edit}
					drag={(event, group) => startDrag(event, { kind: 'ColumnGroup', group })} />
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
							aria-selected={selected?.column === column.id}
							class:selected={selected?.column === column.id}
							onclick={(event) => selectHeader(event, () => selectColumn(column))}
							onkeydown={(event) => selectHeader(event, () => selectColumn(column))}
							aria-colindex={column.axis_index + 2}
							class:requested={snapshot.playback.desired_column === column.id}
							class:installed={observation?.installed?.columns.includes(column.id) === true}
							class:submitted={observation?.submitted?.columns.includes(column.id) === true}
							style:top={`${projection.top + projection.groupHeight}rem`}
							style:left={`${projection.columnOrigin + column.axis_index * projection.column}rem`}>
							<GridLabel
								label={column.label}
								selected={selected?.column === column.id}
								{disabled}
								select={() => selectColumn(column)}
								rename={async (label) =>
									Boolean(await edit({ kind: 'RenameColumn', column: column.id, label }))} />
							<div class="column-state">
								<ParameterTrigger
									{disabled}
									owner={column.id}
									field="play"
									icon={playIcon}
									label={`Launch ${column.label}`} />
								<ParameterTrigger
									owner={column.id}
									field="stop"
									label={`Stop ${column.label}`}
									{disabled} />
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
						style:top={`${projection.rowOrigin + (snapshot.row_start + index) * projection.row}rem`}>
						<div
							class="row-label"
							data-matrix-drop="Item"
							data-row-id={row.item.id.id}
							draggable={!disabled}
							ondragstart={(event) => startDrag(event, { kind: 'Item', item: row.item.id })}
							role="rowheader"
							tabindex="0"
							aria-label={row.item.label}
							aria-selected={selected?.item?.id === row.item.id.id}
							class:selected={selected?.item?.id === row.item.id.id}
							onclick={(event) => selectHeader(event, () => selectRow(row))}
							onkeydown={(event) => selectHeader(event, () => selectRow(row))}
							style:left={`${projection.left}rem`}
							title={row.item.label}>
							<div class="row-title" style:padding-inline-start={`${row.depth * 0.8}rem`}>
								{#if row.item.id.kind === 'Group'}<img src={groupIcon} alt="" />{/if}
								<div class="row-title-label">
									<GridLabel
										label={row.item.label}
										selected={selected?.item?.id === row.item.id.id}
										{disabled}
										select={() => selectRow(row)}
										rename={async (label) =>
											Boolean(await edit({ kind: 'RenameItem', item: row.item.id, label }))} />
								</div>
								<ParameterTrigger
									owner={row.item.id.id}
									field="stop"
									label={`Stop ${row.item.label}`}
									{disabled} />
							</div>
							<RowControls uuid={row.item.id.id} label={row.item.label} />
						</div>
						{#each snapshot.columns as column, columnIndex (column.id)}
							{@const cell = cells.get(`${row.item.id.id}:${column.id}`)}
							<SourceDropZone
								disabled={disabled || row.item.id.kind !== 'Layer'}
								addLabel="Set media"
								replaceOnly={cell?.cell.media ? column.id : null}
								data-column-id={column.id}
								data-cell-drop={row.item.id.kind === 'Layer' ? '' : undefined}
								data-cell-layer={row.item.id.id}
								data-cell-column={column.id}
								style={`position:absolute;top:0;left:${projection.columnOrigin + column.axis_index * projection.column}rem;width:var(--column);height:var(--row)`}
								onsource={(source, _target, event) => drop(row, column, source, event.altKey)}>
								<button
									class="cell"
									draggable={cell != null && row.item.id.kind === 'Layer' && !disabled}
									data-cell-id={cell?.cell.id}
									ondragstart={(event) => {
										if (cell) startCellDrag(event, cell.cell.id);
									}}
									class:has-preview={cell?.source_preview != null && !cell?.source_missing}
									class:group-cell={row.item.id.kind === 'Group'}
									role="gridcell"
									{disabled}
									aria-selected={selected?.item?.id === row.item.id.id &&
										selected?.column === column.id}
									aria-colindex={column.axis_index + 2}
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
									{#if row.item.id.kind === 'Layer' && !cell?.cell.media}<span
											class="empty-icon"
											aria-hidden="true"
											style:mask-image={`url("${emptyIcon}")`}></span
										>{/if}
									<span class="source-label" title={cell?.source_label ?? undefined}
										>{row.item.id.kind === 'Group'
											? ''
											: cell?.source_missing
												? 'Missing source'
												: (cell?.source_label ?? '')}</span>
								</button>
								{#if cell}
									<span class="cell-controls">
										<ParameterTrigger
											owner={cell.cell.id}
											field="play"
											icon={playIcon}
											label={`Play ${row.item.label} / ${column.label}`}
											{disabled} />
										<ParameterTrigger
											owner={cell.cell.id}
											field="stop"
											label={`Stop ${row.item.label} / ${column.label}`}
											{disabled} />
									</span>
								{/if}
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
														assignments: [
															{ layer: row.item.id.id, column: column.id, source: null }
														]
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
		<CellPreviews {snapshot} {metrics} />
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
	.viewport-container {
		flex: 1;
		min-height: 0;
		min-width: 0;
		position: relative;
		overflow: hidden;
	}
	.viewport {
		height: 100%;
		overflow: auto;
		scrollbar-gutter: stable;
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
		justify-content: flex-start;
		gap: 0.3rem;
		border: 0.05rem solid var(--gc-color-border);
	}
	.row-label img {
		width: 1.2em;
		height: 1.2em;
	}
	.row-title {
		display: flex;
		align-items: flex-start;
		gap: 0.25rem;
		min-inline-size: 0;
		box-sizing: border-box;
	}
	.row-title-label {
		flex: 1;
		min-inline-size: 0;
	}
	.row-label.selected,
	.heading.selected {
		box-shadow: inset 0 0 0 0.08rem var(--gc-color-focus);
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
		opacity: 0;
		pointer-events: none;
	}
	:global(.source-drop-zone:hover) .add,
	:global(.source-drop-zone:focus-within) .add,
	:global(.source-drop-zone:hover) .remove,
	:global(.source-drop-zone:focus-within) .remove {
		opacity: 1;
		pointer-events: auto;
	}
	.empty-icon {
		position: absolute;
		left: 50%;
		top: 50%;
		transform: translate(-50%, -50%);
		background: currentColor;
		mask-size: contain;
		width: 1.4rem;
		height: 1.4rem;
		opacity: 0.2;
	}
	:global(.source-drop-zone:hover) .empty-icon,
	:global(.source-drop-zone:focus-within) .empty-icon {
		opacity: 0;
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
	.cell-controls {
		position: absolute;
		z-index: 1;
		left: 0.25rem;
		top: 0.25rem;
		display: flex;
		gap: 0.15rem;
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
