<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { CanvasRenderScheduler } from './canvas-render-scheduler';
	import type { SoundCardMatrixEndpoint, SoundCardRouteRecord } from './sound-card-editor-model';
	import {
		SoundCardRouteMutationController,
		type SoundCardRouteEditSession
	} from './sound-card-route-mutations';

	type PendingMutation =
		| {
				kind: 'add';
				sourceKey: string;
				destinationKey: string;
				gainDb: number;
		  }
		| {
				kind: 'remove';
				sourceKey: string;
				destinationKey: string;
				routeId: number;
		  }
		| {
				kind: 'gain';
				sourceKey: string;
				destinationKey: string;
				routeId: number;
				gainDb: number;
		  };

	type ProjectedRoute = Pick<
		SoundCardRouteRecord,
		'id' | 'sourceKey' | 'destinationKey' | 'gainDb'
	>;

	let {
		title,
		rows,
		sources,
		destinations,
		parent,
		nodeType,
		sourceDeclId,
		destinationDeclId,
		sourceLabel = 'Source',
		destinationLabel = 'Destination',
		active = true,
		inactiveLabel = 'Input is disabled. Authored routes remain stored but carry no live signal.',
		emptyLabel = 'No routes are authored.',
		controller = new SoundCardRouteMutationController()
	} = $props<{
		title: string;
		rows: readonly SoundCardRouteRecord[];
		sources: readonly SoundCardMatrixEndpoint[];
		destinations: readonly SoundCardMatrixEndpoint[];
		parent: number | null;
		nodeType: string;
		sourceDeclId: string;
		destinationDeclId: string;
		sourceLabel?: string;
		destinationLabel?: string;
		active?: boolean;
		inactiveLabel?: string;
		emptyLabel?: string;
		controller?: SoundCardRouteMutationController;
	}>();

	let canvas: HTMLCanvasElement | null = $state(null);
	let canvasWidth = $state(0);
	let canvasHeight = $state(0);
	let scheduler: CanvasRenderScheduler | null = null;
	let resizeObserver: ResizeObserver | null = null;
	let renderReady = $state(false);
	let selectedSourceKey = $state('');
	let selectedDestinationKey = $state('');
	let newGainDb = $state(0);
	let focusedRow = $state(0);
	let focusedColumn = $state(0);
	let pending = $state<Record<string, PendingMutation>>({});
	let gainDrafts = $state<Record<number, string>>({});
	let mutationError = $state<string | null>(null);
	let dragSession: SoundCardRouteEditSession | null = null;
	let dragTail: Promise<void> = Promise.resolve();
	let dragVisited = new Set<string>();
	let dragShouldCreate = true;

	const cellKey = (sourceKey: string, destinationKey: string): string =>
		`${sourceKey}\u001f${destinationKey}`;

	const routeForCell = (sourceKey: string, destinationKey: string): SoundCardRouteRecord | null =>
		rows.find(
			(row: SoundCardRouteRecord) =>
				row.sourceKey === sourceKey && row.destinationKey === destinationKey
		) ?? null;

	const updatePending = (key: string, mutation: PendingMutation | null): void => {
		const next = { ...pending };
		if (mutation) next[key] = mutation;
		else delete next[key];
		pending = next;
	};

	const projectedRoutes = (): readonly ProjectedRoute[] => {
		const projected = new Map<string, ProjectedRoute>();
		for (const row of rows) {
			projected.set(cellKey(row.sourceKey, row.destinationKey), row);
		}
		for (const [key, mutation] of Object.entries(pending)) {
			if (mutation.kind === 'remove') {
				projected.delete(key);
			} else if (mutation.kind === 'add') {
				projected.set(key, {
					id: -1,
					sourceKey: mutation.sourceKey,
					destinationKey: mutation.destinationKey,
					gainDb: mutation.gainDb
				});
			} else {
				const route = projected.get(key);
				if (route) projected.set(key, { ...route, gainDb: mutation.gainDb });
			}
		}
		return [...projected.values()];
	};

	const projectedRouteForCell = (
		sourceKey: string,
		destinationKey: string
	): ProjectedRoute | null =>
		projectedRoutes().find(
			(route) => route.sourceKey === sourceKey && route.destinationKey === destinationKey
		) ?? null;

	const displayedRows = (): readonly SoundCardRouteRecord[] =>
		rows
			.filter((row: SoundCardRouteRecord) => {
				const mutation = pending[cellKey(row.sourceKey, row.destinationKey)];
				return mutation?.kind !== 'remove';
			})
			.map((row: SoundCardRouteRecord) => {
				const mutation = pending[cellKey(row.sourceKey, row.destinationKey)];
				return mutation?.kind === 'gain' ? { ...row, gainDb: mutation.gainDb } : row;
			});

	const reconcilePending = (): void => {
		let changed = false;
		const next = { ...pending };
		for (const [key, mutation] of Object.entries(pending)) {
			const route = routeForCell(mutation.sourceKey, mutation.destinationKey);
			const settled =
				(mutation.kind === 'add' && route !== null) ||
				(mutation.kind === 'remove' && route === null) ||
				(mutation.kind === 'gain' && route?.gainDb === mutation.gainDb);
			if (settled) {
				delete next[key];
				changed = true;
			}
		}
		if (changed) pending = next;
	};

	const endpointByKey = (
		endpoints: readonly SoundCardMatrixEndpoint[],
		key: string
	): SoundCardMatrixEndpoint | null => endpoints.find((endpoint) => endpoint.key === key) ?? null;

	const createRoute = async (
		source: SoundCardMatrixEndpoint,
		destination: SoundCardMatrixEndpoint,
		gainDb: number
	): Promise<boolean> => {
		if (parent === null) return false;
		const key = cellKey(source.key, destination.key);
		updatePending(key, {
			kind: 'add',
			sourceKey: source.key,
			destinationKey: destination.key,
			gainDb
		});
		const success = await controller.create({
			parent,
			nodeType,
			sourceDeclId,
			source: source.value,
			destinationDeclId,
			destination: destination.value,
			gainDb
		});
		if (!success) {
			updatePending(key, null);
			mutationError = `Could not create the ${title} route. The backend rejected the edit.`;
		}
		return success;
	};

	const removeRoute = async (route: SoundCardRouteRecord): Promise<boolean> => {
		const key = cellKey(route.sourceKey, route.destinationKey);
		updatePending(key, {
			kind: 'remove',
			sourceKey: route.sourceKey,
			destinationKey: route.destinationKey,
			routeId: route.id
		});
		const success = await controller.remove(route.id);
		if (!success) {
			updatePending(key, null);
			mutationError = `Could not remove the ${title} route. The backend rejected the edit.`;
		}
		return success;
	};

	const setRouteGain = async (route: SoundCardRouteRecord, gainDb: number): Promise<boolean> => {
		if (route.gainParameterId === null) return false;
		const key = cellKey(route.sourceKey, route.destinationKey);
		updatePending(key, {
			kind: 'gain',
			sourceKey: route.sourceKey,
			destinationKey: route.destinationKey,
			routeId: route.id,
			gainDb
		});
		const success = await controller.setGain({
			parameter: route.gainParameterId,
			gainDb,
			behaviour: route.gainEventBehaviour
		});
		if (!success) {
			updatePending(key, null);
			gainDrafts = { ...gainDrafts, [route.id]: String(route.gainDb ?? 0) };
			mutationError = `Could not update the ${title} gain. The backend rejected the edit.`;
		}
		return success;
	};

	const addSelectedRoute = async (): Promise<void> => {
		const source = endpointByKey(sources, selectedSourceKey);
		const destination = endpointByKey(destinations, selectedDestinationKey);
		if (!source || !destination || projectedRouteForCell(source.key, destination.key)) return;
		mutationError = null;
		await createRoute(source, destination, newGainDb);
	};

	const commitGain = async (route: SoundCardRouteRecord): Promise<void> => {
		const draft = Number(gainDrafts[route.id] ?? route.gainDb ?? 0);
		if (!Number.isFinite(draft)) {
			gainDrafts = { ...gainDrafts, [route.id]: String(route.gainDb ?? 0) };
			return;
		}
		const gainDb = Math.max(-120, Math.min(24, draft));
		gainDrafts = { ...gainDrafts, [route.id]: String(gainDb) };
		if (gainDb === route.gainDb) return;
		mutationError = null;
		await setRouteGain(route, gainDb);
	};

	const routeAt = (row: number, column: number): SoundCardRouteRecord | null => {
		const source = sources[row];
		const destination = destinations[column];
		return source && destination ? routeForCell(source.key, destination.key) : null;
	};

	const toggleCell = async (row: number, column: number, shouldCreate?: boolean): Promise<void> => {
		const source = sources[row];
		const destination = destinations[column];
		if (!source || !destination) return;
		const existing = routeForCell(source.key, destination.key);
		const projected = projectedRouteForCell(source.key, destination.key);
		const create = shouldCreate ?? projected === null;
		if (create && projected === null) {
			await createRoute(source, destination, 0);
		} else if (!create && existing) {
			await removeRoute(existing);
		}
	};

	const cellFromPointer = (event: PointerEvent): { row: number; column: number } | null => {
		if (!canvas || sources.length === 0 || destinations.length === 0) return null;
		const bounds = canvas.getBoundingClientRect();
		if (bounds.width <= 0 || bounds.height <= 0) return null;
		return {
			row: Math.max(
				0,
				Math.min(
					sources.length - 1,
					Math.floor(((event.clientY - bounds.top) / bounds.height) * sources.length)
				)
			),
			column: Math.max(
				0,
				Math.min(
					destinations.length - 1,
					Math.floor(((event.clientX - bounds.left) / bounds.width) * destinations.length)
				)
			)
		};
	};

	const queueDragCell = (row: number, column: number): void => {
		const source = sources[row];
		const destination = destinations[column];
		if (!source || !destination) return;
		const key = cellKey(source.key, destination.key);
		if (dragVisited.has(key)) return;
		dragVisited.add(key);
		focusedRow = row;
		focusedColumn = column;
		dragTail = dragTail.then(() => toggleCell(row, column, dragShouldCreate));
	};

	const startMatrixDrag = (event: PointerEvent): void => {
		const cell = cellFromPointer(event);
		if (!cell || !canvas || parent === null) return;
		event.preventDefault();
		canvas.setPointerCapture(event.pointerId);
		const current = projectedRouteForCell(sources[cell.row].key, destinations[cell.column].key);
		dragShouldCreate = current === null;
		dragVisited = new Set();
		const session = controller.createEditSession(
			`${dragShouldCreate ? 'Add' : 'Remove'} ${title} routes`
		);
		dragSession = session;
		dragTail = session.begin();
		queueDragCell(cell.row, cell.column);
	};

	const continueMatrixDrag = (event: PointerEvent): void => {
		if (!dragSession) return;
		const cell = cellFromPointer(event);
		if (cell) queueDragCell(cell.row, cell.column);
	};

	const finishMatrixDrag = (event: PointerEvent): void => {
		if (!canvas || !dragSession) return;
		if (canvas.hasPointerCapture(event.pointerId)) canvas.releasePointerCapture(event.pointerId);
		const session = dragSession;
		dragSession = null;
		dragTail = dragTail.finally(() => session.end());
	};

	const handleMatrixKey = (event: KeyboardEvent): void => {
		if (sources.length === 0 || destinations.length === 0) return;
		if (event.key === 'ArrowUp') focusedRow = Math.max(0, focusedRow - 1);
		else if (event.key === 'ArrowDown') focusedRow = Math.min(sources.length - 1, focusedRow + 1);
		else if (event.key === 'ArrowLeft') focusedColumn = Math.max(0, focusedColumn - 1);
		else if (event.key === 'ArrowRight') {
			focusedColumn = Math.min(destinations.length - 1, focusedColumn + 1);
		} else if (event.key === 'Enter' || event.key === ' ') {
			void toggleCell(focusedRow, focusedColumn);
		} else return;
		event.preventDefault();
	};

	const draw = (): void => {
		if (!canvas || canvasWidth <= 0 || canvasHeight <= 0) return;
		const context = canvas.getContext('2d');
		if (!context) return;
		const ratio = Math.max(1, globalThis.devicePixelRatio ?? 1);
		const width = Math.max(1, Math.round(canvasWidth * ratio));
		const height = Math.max(1, Math.round(canvasHeight * ratio));
		if (canvas.width !== width) canvas.width = width;
		if (canvas.height !== height) canvas.height = height;
		context.setTransform(ratio, 0, 0, ratio, 0, 0);
		context.clearRect(0, 0, canvasWidth, canvasHeight);
		context.fillStyle = '#141a25';
		context.fillRect(0, 0, canvasWidth, canvasHeight);
		if (sources.length === 0 || destinations.length === 0) return;

		const cellWidth = canvasWidth / destinations.length;
		const cellHeight = canvasHeight / sources.length;
		context.strokeStyle = '#2b3547';
		context.lineWidth = 0.7;
		const rowStep = Math.max(1, Math.ceil(sources.length / 32));
		const columnStep = Math.max(1, Math.ceil(destinations.length / 32));
		for (let row = rowStep; row < sources.length; row += rowStep) {
			const y = row * cellHeight;
			context.beginPath();
			context.moveTo(0, y);
			context.lineTo(canvasWidth, y);
			context.stroke();
		}
		for (let column = columnStep; column < destinations.length; column += columnStep) {
			const x = column * cellWidth;
			context.beginPath();
			context.moveTo(x, 0);
			context.lineTo(x, canvasHeight);
			context.stroke();
		}

		context.fillStyle = active ? '#62b5f3' : '#69768a';
		for (const route of projectedRoutes()) {
			const row = sources.findIndex(
				(source: SoundCardMatrixEndpoint) => source.key === route.sourceKey
			);
			const column = destinations.findIndex(
				(destination: SoundCardMatrixEndpoint) => destination.key === route.destinationKey
			);
			if (row < 0 || column < 0) continue;
			const inset = Math.min(1.5, cellWidth * 0.14, cellHeight * 0.14);
			context.fillRect(
				column * cellWidth + inset,
				row * cellHeight + inset,
				Math.max(0.5, cellWidth - inset * 2),
				Math.max(0.5, cellHeight - inset * 2)
			);
		}

		context.strokeStyle = '#f4cf75';
		context.lineWidth = 1.5;
		context.strokeRect(
			focusedColumn * cellWidth + 0.75,
			focusedRow * cellHeight + 0.75,
			Math.max(0, cellWidth - 1.5),
			Math.max(0, cellHeight - 1.5)
		);
	};

	const requestDraw = (): void => scheduler?.request(draw);

	$effect(() => {
		if (!sources.some((source: SoundCardMatrixEndpoint) => source.key === selectedSourceKey)) {
			selectedSourceKey = sources[0]?.key ?? '';
		}
		if (
			!destinations.some(
				(destination: SoundCardMatrixEndpoint) => destination.key === selectedDestinationKey
			)
		) {
			selectedDestinationKey = destinations[0]?.key ?? '';
		}
		focusedRow = Math.min(focusedRow, Math.max(0, sources.length - 1));
		focusedColumn = Math.min(focusedColumn, Math.max(0, destinations.length - 1));
	});

	$effect(() => {
		rows;
		reconcilePending();
	});

	$effect(() => {
		rows;
		sources;
		destinations;
		pending;
		focusedRow;
		focusedColumn;
		active;
		canvasWidth;
		canvasHeight;
		renderReady;
		requestDraw();
	});

	onMount(() => {
		if (!canvas) return;
		scheduler = new CanvasRenderScheduler(requestAnimationFrame, cancelAnimationFrame);
		resizeObserver = new ResizeObserver(([entry]) => {
			canvasWidth = entry.contentRect.width;
			canvasHeight = entry.contentRect.height;
		});
		resizeObserver.observe(canvas);
		renderReady = true;
	});

	onDestroy(() => {
		resizeObserver?.disconnect();
		scheduler?.dispose();
		if (dragSession) {
			const session = dragSession;
			dragSession = null;
			void dragTail.finally(() => session.end());
		}
	});
</script>

<section class="route-matrix" class:inactive={!active} aria-label={title}>
	<header>
		<div>
			<h3>{title}</h3>
			<p>
				{sources.length}
				{sourceLabel.toLowerCase()} × {destinations.length}
				{destinationLabel.toLowerCase()}
			</p>
		</div>
		<span>{projectedRoutes().length} authored</span>
	</header>

	{#if !active}
		<p class="inactive-notice" role="status">{inactiveLabel}</p>
	{/if}

	{#if sources.length > 0 && destinations.length > 0}
		<div class="canvas-frame">
			<canvas
				bind:this={canvas}
				role="grid"
				tabindex="0"
				aria-label="{title}. Arrow keys move the focused cell; Enter or Space toggles it."
				aria-rowcount={sources.length}
				aria-colcount={destinations.length}
				onkeydown={handleMatrixKey}
				onpointerdown={startMatrixDrag}
				onpointermove={continueMatrixDrag}
				onpointerup={finishMatrixDrag}
				onpointercancel={finishMatrixDrag}>
				{title}. {sources.length} rows, {destinations.length} columns, {projectedRoutes().length}
				authored routes.
			</canvas>
			<p class="focused-cell" aria-live="polite">
				{sources[focusedRow]?.label} → {destinations[focusedColumn]?.label}:
				{projectedRouteForCell(
					sources[focusedRow]?.key ?? '',
					destinations[focusedColumn]?.key ?? ''
				)
					? 'authored'
					: 'empty'}
			</p>
		</div>

		<form
			class="route-create"
			onsubmit={(event) => {
				event.preventDefault();
				void addSelectedRoute();
			}}>
			<label>
				<span>{sourceLabel}</span>
				<select bind:value={selectedSourceKey}>
					{#each sources as source (source.key)}
						<option value={source.key}>{source.label}</option>
					{/each}
				</select>
			</label>
			<label>
				<span>{destinationLabel}</span>
				<select bind:value={selectedDestinationKey}>
					{#each destinations as destination (destination.key)}
						<option value={destination.key}>{destination.label}</option>
					{/each}
				</select>
			</label>
			<label>
				<span>Gain (dB)</span>
				<input type="number" min="-120" max="24" step="0.1" bind:value={newGainDb} />
			</label>
			<button
				type="submit"
				disabled={parent === null ||
					projectedRouteForCell(selectedSourceKey, selectedDestinationKey) !== null}>
				Add route
			</button>
		</form>
	{:else}
		<p class="empty">The current device or channel list does not expose both matrix axes.</p>
	{/if}

	{#if displayedRows().length > 0}
		<div class="route-list">
			<table>
				<thead>
					<tr>
						<th scope="col">{sourceLabel}</th>
						<th scope="col">{destinationLabel}</th>
						<th scope="col">Gain</th>
						<th scope="col"><span class="visually-hidden">Actions</span></th>
					</tr>
				</thead>
				<tbody>
					{#each displayedRows() as route (route.id)}
						<tr>
							<td>{route.source}</td>
							<td>{route.destination}</td>
							<td>
								<input
									aria-label="Gain for {route.source} to {route.destination}"
									type="number"
									min="-120"
									max="24"
									step="0.1"
									value={gainDrafts[route.id] ?? String(route.gainDb ?? 0)}
									oninput={(event) => {
										gainDrafts = {
											...gainDrafts,
											[route.id]: (event.currentTarget as HTMLInputElement).value
										};
									}}
									onchange={() => void commitGain(route)} />
							</td>
							<td>
								<button
									type="button"
									class="remove"
									onclick={() => void removeRoute(route)}
									aria-label="Remove route from {route.source} to {route.destination}">
									Remove
								</button>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{:else}
		<p class="empty">{emptyLabel}</p>
	{/if}

	{#if mutationError}
		<div class="mutation-error" role="alert">
			<span>{mutationError}</span>
			<button type="button" onclick={() => (mutationError = null)}>Dismiss</button>
		</div>
	{/if}
</section>

<style>
	.route-matrix {
		display: grid;
		gap: 0.6rem;
		min-inline-size: 0;
		padding: 0.65rem;
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.45rem;
		background: var(--gc-color-bg-light);
	}

	header,
	.mutation-error {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.7rem;
	}

	h3,
	p {
		margin: 0;
	}

	h3 {
		font-size: 0.85rem;
	}

	header p,
	header > span,
	.empty,
	.focused-cell {
		color: var(--gc-color-text-muted);
		font-size: 0.7rem;
	}

	.canvas-frame {
		display: grid;
		gap: 0.3rem;
	}

	canvas {
		display: block;
		inline-size: 100%;
		block-size: min(22rem, 42vh);
		min-block-size: 10rem;
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.35rem;
		background: #141a25;
		touch-action: none;
		cursor: crosshair;
	}

	canvas:focus-visible {
		outline: 0.15rem solid var(--gc-color-accent);
		outline-offset: 0.1rem;
	}

	.route-create {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr)) auto;
		align-items: end;
		gap: 0.45rem;
	}

	label {
		display: grid;
		gap: 0.2rem;
		color: var(--gc-color-text-muted);
		font-size: 0.68rem;
	}

	select,
	input,
	button {
		min-block-size: 2rem;
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.3rem;
		background: var(--gc-color-bg-lighter);
		color: var(--gc-color-text);
		font: inherit;
	}

	select,
	input {
		min-inline-size: 0;
		padding-inline: 0.4rem;
	}

	button {
		padding-inline: 0.65rem;
		cursor: pointer;
	}

	button:hover:not(:disabled) {
		border-color: var(--gc-color-accent);
	}

	button:disabled {
		opacity: 0.48;
		cursor: not-allowed;
	}

	.route-list {
		max-block-size: 16rem;
		overflow: auto;
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.35rem;
	}

	table {
		inline-size: 100%;
		border-collapse: collapse;
		font-size: 0.73rem;
	}

	th,
	td {
		padding: 0.38rem 0.45rem;
		border-block-end: 0.0625rem solid var(--gc-color-border);
		text-align: start;
		overflow-wrap: anywhere;
	}

	th {
		position: sticky;
		inset-block-start: 0;
		z-index: 1;
		background: var(--gc-color-bg-lighter);
		color: var(--gc-color-text-muted);
	}

	tbody tr {
		content-visibility: auto;
		contain-intrinsic-block-size: 2.5rem;
	}

	td input {
		inline-size: 6rem;
	}

	.remove {
		min-block-size: 1.7rem;
		color: #ffabab;
	}

	.inactive canvas {
		opacity: 0.72;
	}

	.inactive-notice,
	.empty,
	.mutation-error {
		padding: 0.55rem;
		border-radius: 0.3rem;
	}

	.inactive-notice {
		background: #3a3020;
		color: #e9bd70;
		font-size: 0.74rem;
	}

	.mutation-error {
		background: #3c2328;
		color: #ffb4b4;
		font-size: 0.74rem;
	}

	.mutation-error button {
		min-block-size: 1.6rem;
	}

	.visually-hidden {
		position: absolute;
		inline-size: 0.0625rem;
		block-size: 0.0625rem;
		padding: 0;
		margin: -0.0625rem;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}

	@media (max-width: 50rem) {
		.route-create {
			grid-template-columns: 1fr 1fr;
		}
	}

	@media (max-width: 32rem) {
		.route-create {
			grid-template-columns: 1fr;
		}
	}
</style>
