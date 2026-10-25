<script lang="ts" generics="T">
	import { onDestroy, tick, untrack, type Snippet } from 'svelte';
	import { RowHeightIndex, type RowKey } from './height-index';
	let {
		rows,
		keyOf,
		scroller,
		rowHeightRem = 1.8,
		variableHeights = false,
		item
	}: {
		rows: readonly T[];
		keyOf: (row: T) => RowKey;
		scroller: HTMLElement | null;
		rowHeightRem?: number;
		variableHeights?: boolean;
		item: Snippet<[T, number]>;
	} = $props();
	const index = new RowHeightIndex();
	let revision = $state(0);
	let metric = $state<HTMLDivElement | null>(null);
	let region = $state<HTMLDivElement | null>(null);
	let rowHeightPx = $state(28.8);
	let scrollTop = $state(0);
	let viewportHeight = $state(0);
	let focusedKey = $state<RowKey | null>(null);
	let publishedRows = $state.raw<readonly T[]>([]);
	let restoreGeneration = 0;
	let revealAnchor: RowKey | undefined;
	const rowElements = new Map<HTMLElement, RowKey>();
	let measurementObserver: ResizeObserver | null = null;

	const anchor = () => {
		const row =
			(revealAnchor === undefined ? undefined : index.indexOf(revealAnchor)) ??
			index.rowAt(scroller?.scrollTop ?? scrollTop);
		return { key: index.keyAt(row), inset: (scroller?.scrollTop ?? scrollTop) - index.offset(row) };
	};
	const restore = (bookmark: ReturnType<typeof anchor>) => {
		if (!scroller || bookmark.key === undefined) return;
		const row = index.indexOf(bookmark.key);
		if (row !== undefined) scroller.scrollTop = Math.max(0, index.offset(row) + bookmark.inset);
		scrollTop = scroller.scrollTop;
	};
	const restoreAfterFlush = (bookmark: ReturnType<typeof anchor>) => {
		const generation = ++restoreGeneration;
		void tick().then(() => {
			if (generation === restoreGeneration) restore(bookmark);
		});
	};

	$effect.pre(() => {
		const targetRows = rows,
			keys = targetRows.map(keyOf),
			estimate = rowHeightPx,
			element = scroller;
		untrack(() => {
			const bookmark = anchor();
			if (focusedKey !== null && !keys.includes(focusedKey)) {
				if (region?.contains(document.activeElement)) element?.focus({ preventScroll: true });
				focusedKey = null;
			}
			if (index.synchronize(keys, estimate)) {
				if (revealAnchor !== undefined && index.indexOf(revealAnchor) === undefined)
					revealAnchor = undefined;
				revision += 1;
				// Restore after the new scroll extent reaches the DOM.
				restoreAfterFlush(bookmark);
			}
			publishedRows = targetRows;
		});
	});

	let range = $derived.by(() => {
		revision;
		return index.window(scrollTop, viewportHeight);
	});
	let mountedRows = $derived.by(() => {
		revision;
		const indices = Array.from(
			{ length: range.end - range.start },
			(_, offset) => range.start + offset
		);
		const pinned = focusedKey === null ? undefined : index.indexOf(focusedKey);
		if (pinned !== undefined && (pinned < range.start || pinned >= range.end)) indices.push(pinned);
		return indices
			.sort((a, b) => a - b)
			.map((row) => ({ row: publishedRows[row], position: row, top: index.offset(row) }));
	});
	let totalHeight = $derived.by(() => {
		revision;
		return index.total;
	});

	export function reveal(key: RowKey, align: 'nearest' | 'center' = 'nearest'): boolean {
		const row = index.indexOf(key);
		if (!scroller || row === undefined) return false;
		revealAnchor = key;
		const top = index.offset(row),
			bottom = index.offset(row + 1);
		if (align === 'center')
			scroller.scrollTop = Math.max(0, (top + bottom - scroller.clientHeight) / 2);
		else if (top < scroller.scrollTop) scroller.scrollTop = top;
		else if (bottom > scroller.scrollTop + scroller.clientHeight)
			scroller.scrollTop = bottom - scroller.clientHeight;
		scrollTop = scroller.scrollTop;
		return true;
	}

	$effect(() => {
		if (!scroller || !metric) return;
		const element = scroller,
			rowMetric = metric;
		const measureViewport = () => {
			viewportHeight = element.clientHeight;
			rowHeightPx = rowMetric.getBoundingClientRect().height || rowHeightPx;
			scrollTop = element.scrollTop;
		};
		const scroll = () => {
			scrollTop = element.scrollTop;
		};
		const releaseAnchor = () => {
			revealAnchor = undefined;
		};
		untrack(measureViewport);
		element.addEventListener('scroll', scroll, { passive: true });
		for (const event of ['wheel', 'pointerdown', 'touchstart', 'keydown'])
			element.addEventListener(event, releaseAnchor, { passive: true });
		const observer = new ResizeObserver(() => untrack(measureViewport));
		observer.observe(element);
		observer.observe(rowMetric);
		return () => {
			observer.disconnect();
			element.removeEventListener('scroll', scroll);
			for (const event of ['wheel', 'pointerdown', 'touchstart', 'keydown'])
				element.removeEventListener(event, releaseAnchor);
		};
	});
	$effect(() => {
		if (!variableHeights) return;
		measurementObserver = new ResizeObserver((entries) => {
			const bookmark = anchor();
			let changed = false;
			for (const entry of entries) {
				const key = rowElements.get(entry.target as HTMLElement);
				const height =
					entry.borderBoxSize[0]?.blockSize ?? entry.target.getBoundingClientRect().height;
				if (key !== undefined && height > 0) changed = index.measure(key, height) || changed;
			}
			if (changed) {
				revision += 1;
				restoreAfterFlush(bookmark);
			}
		});
		for (const element of rowElements.keys()) measurementObserver.observe(element);
		return () => {
			measurementObserver?.disconnect();
			measurementObserver = null;
		};
	});
	const measureRow = (element: HTMLElement, key: RowKey) => {
		rowElements.set(element, key);
		measurementObserver?.observe(element);
		return {
			destroy() {
				measurementObserver?.unobserve(element);
				rowElements.delete(element);
			}
		};
	};
	onDestroy(() => {
		restoreGeneration += 1;
		measurementObserver?.disconnect();
	});
</script>

<div
	class="virtual-rows"
	bind:this={region}
	style:block-size={`${totalHeight}px`}
	style:--row-height={`${rowHeightRem}rem`}
	data-row-count={publishedRows.length}
	data-mounted-row-count={mountedRows.length}
	onfocusout={(event) => {
		if (!(event.relatedTarget instanceof Node) || !scroller?.contains(event.relatedTarget))
			focusedKey = null;
	}}>
	<div class="row-metric" bind:this={metric} aria-hidden="true"></div>
	{#each mountedRows as entry (keyOf(entry.row))}
		<div
			class="virtual-row"
			class:fixed={!variableHeights}
			style:top={`${entry.top}px`}
			use:measureRow={keyOf(entry.row)}
			onfocusin={() => (focusedKey = keyOf(entry.row))}>
			{@render item(entry.row, entry.position)}
		</div>
	{/each}
</div>

<style>
	.virtual-rows {
		position: relative;
		min-inline-size: 0;
		overflow-anchor: none;
	}
	.row-metric {
		position: absolute;
		block-size: var(--row-height);
		pointer-events: none;
	}
	.virtual-row {
		display: flex;
		align-items: center;
		position: absolute;
		inset-inline: 0;
		box-sizing: border-box;
		min-block-size: var(--row-height);
	}
	.virtual-row.fixed {
		block-size: var(--row-height);
		overflow: hidden;
		display: flex;
		align-items: center;
	}
</style>
