<script lang="ts">
	import { onMount } from 'svelte';
	import type { NodeId, UiLogRecord } from '../../../types';
	import type { PanelProps, PanelState } from '../../../dockview/panel-types';
	import { appState } from '../../../store/workbench.svelte';
	import {
		DEFAULT_LOG_UI_UPDATE_HZ,
		MAX_LOG_UI_UPDATE_HZ,
		MIN_LOG_UI_UPDATE_HZ,
		normalizeLogUiUpdateHz
	} from '../../../store/logger-ui-config';
	import { sendClearLogsIntent, sendSetLogMaxEntriesIntent } from '../../../store/ui-intents';
	import { copyTextToClipboard } from '../../../utils/clipboard';
	import { text } from '@sveltejs/kit';
	import { fade, slide } from 'svelte/transition';

	let { panelId, panelType, title, params }: PanelProps = $props();

	let panel = $state<PanelState>({
		panelId: '',
		panelType: '',
		title: '',
		params: {}
	});

	$effect(() => {
		panel = {
			panelId,
			panelType,
			title,
			params
		};
	});

	export const setPanelState = (next: PanelState): void => {
		panel = next;
	};

	interface LoggerEntry {
		key: string;
		record: UiLogRecord;
		sourceLabel: string;
		sourceFilterText: string;
		tagFilterText: string;
		contentFilterText: string;
		formattedTime: string;
		repeatCount: number;
	}

	const ENGINE_SOURCE_LABEL = 'engine';
	const LIST_BOTTOM_EPSILON_PX = 6;
	const LIVE_TAIL_RENDER_LIMIT = 200;

	const normalizeSearchText = (value: string): string => value.trim().toLowerCase();

	const repeatCountForRecord = (record: UiLogRecord): number => {
		const raw = Number(record.repeat_count ?? 1);
		if (!Number.isFinite(raw)) {
			return 1;
		}
		return Math.max(1, Math.floor(raw));
	};

	let session = $derived(appState.session);
	let records = $derived(session?.logRecords ?? []);
	let maxEntries = $derived(session?.logMaxEntries ?? 0);
	let logUiUpdateHz = $derived(session?.logUiUpdateHz ?? DEFAULT_LOG_UI_UPDATE_HZ);
	let desiredMaxEntries = $state(128);
	let desiredLogUiUpdateHz = $state(DEFAULT_LOG_UI_UPDATE_HZ);

	let sourceFilter = $state('');
	let tagFilter = $state('');
	let contentFilter = $state('');
	let collapseDuplicates = $state(true);
	let collapseAllDuplicates = $state(false);

	let normalizedSourceFilter = $derived(normalizeSearchText(sourceFilter));
	let normalizedTagFilter = $derived(normalizeSearchText(tagFilter));
	let normalizedContentFilter = $derived(normalizeSearchText(contentFilter));
	let isFiltered = $derived(
		normalizedSourceFilter.length > 0 ||
			normalizedTagFilter.length > 0 ||
			normalizedContentFilter.length > 0
	);

	let loggerList = $state<HTMLDivElement | null>(null);
	let listEndMarker = $state<HTMLDivElement | null>(null);
	let focusedRecordKey = $state<string | null>(null);
	let followLatest = $state(true);
	let viewportSyncRaf: number | null = null;
	let suppressedScrollEvents = 0;
	let userScrollIntentUntilMs = 0;

	let rawEntryLimit = $derived(maxEntries > 0 ? maxEntries : Math.max(1, desiredMaxEntries));
	let effectiveRenderLimit = $derived(
		followLatest && focusedRecordKey === null && !isFiltered
			? Math.min(rawEntryLimit, LIVE_TAIL_RENDER_LIMIT)
			: rawEntryLimit
	);

	interface CachedRecordDecorations {
		timestampMs: number;
		sourceLabel: string;
		sourceFilterText: string;
		tagFilterText: string;
		contentFilterText: string;
		formattedTime: string;
	}

	interface DisplayedEntriesResult {
		entries: LoggerEntry[];
		collapsedGroupCount: number;
	}

	interface CollapsedGroup {
		signature: string;
		key: string;
		record: UiLogRecord;
		repeatCount: number;
		latestRecordIndex: number;
	}

	const sourceLabelCache = new Map<NodeId, string>();
	const recordDecorationsCache = new Map<number, CachedRecordDecorations>();
	let lastSessionRef: typeof session = null;

	let copyButtonText = $state<string | null>(null);
	let overRecordKey = $state<string | null>(null);

	const resolveSourceLabel = (record: UiLogRecord): string => {
		if (record.origin === undefined) {
			return ENGINE_SOURCE_LABEL;
		}

		const cached = sourceLabelCache.get(record.origin);
		if (cached !== undefined) {
			return cached;
		}

		const label = session?.graph.state.nodesById.get(record.origin)?.meta.label;
		const resolved = label ? `${label}` : `node ${record.origin}`;
		sourceLabelCache.set(record.origin, resolved);
		return resolved;
	};

	const formatTimestamp = (timestampMs: number): string => {
		const date = new Date(timestampMs);
		const hours = String(date.getHours()).padStart(2, '0');
		const minutes = String(date.getMinutes()).padStart(2, '0');
		const seconds = String(date.getSeconds()).padStart(2, '0');
		const milliseconds = String(date.getMilliseconds()).padStart(3, '0');
		return `${hours}:${minutes}:${seconds}.${milliseconds}`;
	};

	const decorationsForRecord = (record: UiLogRecord): CachedRecordDecorations => {
		const sourceLabel = resolveSourceLabel(record);
		const sourceFilterText = sourceLabel.toLowerCase();
		const tagFilterText = record.tag.toLowerCase();
		const contentFilterText = record.message.toLowerCase();

		const cached = recordDecorationsCache.get(record.id);
		if (
			cached &&
			cached.timestampMs === record.timestamp_ms &&
			cached.sourceFilterText === sourceFilterText &&
			cached.tagFilterText === tagFilterText &&
			cached.contentFilterText === contentFilterText
		) {
			return cached;
		}

		const next: CachedRecordDecorations = {
			timestampMs: record.timestamp_ms,
			sourceLabel,
			sourceFilterText,
			tagFilterText,
			contentFilterText,
			formattedTime: formatTimestamp(record.timestamp_ms)
		};
		recordDecorationsCache.set(record.id, next);
		return next;
	};

	const makeEntry = (record: UiLogRecord, key: string, repeatCount: number): LoggerEntry => {
		const decorations = decorationsForRecord(record);
		return {
			key,
			record,
			sourceLabel: decorations.sourceLabel,
			sourceFilterText: decorations.sourceFilterText,
			tagFilterText: decorations.tagFilterText,
			contentFilterText: decorations.contentFilterText,
			formattedTime: decorations.formattedTime,
			repeatCount
		};
	};

	const uniqueRecordKey = (recordId: number, duplicateIndex: number): string => {
		if (duplicateIndex <= 0) {
			return `r:${recordId}`;
		}
		return `r:${recordId}:d${duplicateIndex}`;
	};

	const collapseSignatureForRecord = (record: UiLogRecord): string => {
		return JSON.stringify([record.level, record.tag, record.origin ?? null, record.message]);
	};

	const formatLogEntryForClipboard = (entry: LoggerEntry): string => {
		const record = entry.record;
		const headerParts = [
			`[${entry.formattedTime}]`,
			`[${record.level}]`,
			`[${entry.sourceLabel}]`,
			record.tag.trim().length > 0 ? `[${record.tag}]` : null,
			entry.repeatCount > 1 ? `[x${entry.repeatCount}]` : null
		].filter((part): part is string => part !== null);
		const message = record.message;
		const header = headerParts.join(' ');
		return message.length > 0 ? `${header}\n${message}` : header;
	};

	const hasLoggerSelection = (): boolean => {
		if (!loggerList || typeof window === 'undefined') {
			return false;
		}

		const selection = window.getSelection();
		if (!selection || selection.isCollapsed || selection.rangeCount === 0) {
			return false;
		}

		const commonAncestor = selection.getRangeAt(0).commonAncestorContainer;
		const selectionRoot =
			commonAncestor.nodeType === Node.TEXT_NODE ? commonAncestor.parentNode : commonAncestor;

		return selectionRoot instanceof Node && loggerList.contains(selectionRoot);
	};

	$effect(() => {
		if (session !== lastSessionRef) {
			lastSessionRef = session;
			sourceLabelCache.clear();
			recordDecorationsCache.clear();
		}
	});

	$effect(() => {
		if (records.length === 0) {
			recordDecorationsCache.clear();
			return;
		}

		const retainedIds = new Set(records.map((record) => record.id));
		for (const cachedId of [...recordDecorationsCache.keys()]) {
			if (retainedIds.has(cachedId)) {
				continue;
			}
			recordDecorationsCache.delete(cachedId);
		}
	});

	let displayedEntriesResult = $derived.by<DisplayedEntriesResult>(() => {
		const renderLimit = effectiveRenderLimit;
		if (collapseDuplicates) {
			if (collapseAllDuplicates) {
				const groupsBySignature = new Map<string, CollapsedGroup>();

				for (let recordIndex = 0; recordIndex < records.length; recordIndex += 1) {
					const record = records[recordIndex];
					const signature = collapseSignatureForRecord(record);
					const repeatCount = repeatCountForRecord(record);
					const existing = groupsBySignature.get(signature);
					if (existing) {
						existing.record = record;
						existing.latestRecordIndex = recordIndex;
						existing.repeatCount += repeatCount;
					} else {
						groupsBySignature.set(signature, {
							signature,
							key: `g:${record.id}`,
							record,
							repeatCount,
							latestRecordIndex: recordIndex
						});
					}
				}

				const sortedGroups = [...groupsBySignature.values()].sort(
					(left, right) => left.latestRecordIndex - right.latestRecordIndex
				);
				const startIndex = Math.max(0, sortedGroups.length - renderLimit);
				const entries = sortedGroups
					.slice(startIndex)
					.map((group) => makeEntry(group.record, group.key, group.repeatCount));
				return {
					entries,
					collapsedGroupCount: sortedGroups.length
				};
			}

			const entries: LoggerEntry[] = [];
			const duplicateCountById = new Map<number, number>();
			const startIndex = Math.max(0, records.length - renderLimit);
			for (let recordIndex = startIndex; recordIndex < records.length; recordIndex += 1) {
				const record = records[recordIndex];
				const duplicateIndex = duplicateCountById.get(record.id) ?? 0;
				duplicateCountById.set(record.id, duplicateIndex + 1);
				entries.push(
					makeEntry(
						record,
						uniqueRecordKey(record.id, duplicateIndex),
						repeatCountForRecord(record)
					)
				);
			}
			return {
				entries,
				collapsedGroupCount: entries.length
			};
		}

		let remaining = renderLimit;
		const reversedEntries: LoggerEntry[] = [];
		const duplicateCountById = new Map<number, number>();

		for (let recordIndex = records.length - 1; recordIndex >= 0; recordIndex -= 1) {
			if (remaining <= 0) {
				break;
			}

			const record = records[recordIndex];
			const duplicateIndex = duplicateCountById.get(record.id) ?? 0;
			duplicateCountById.set(record.id, duplicateIndex + 1);
			const recordKey = uniqueRecordKey(record.id, duplicateIndex);
			const repeatCount = repeatCountForRecord(record);
			const takeCount = Math.min(remaining, repeatCount);

			for (let offset = 0; offset < takeCount; offset += 1) {
				const occurrenceIndex = repeatCount - 1 - offset;
				reversedEntries.push(makeEntry(record, `${recordKey}:o${occurrenceIndex}`, 1));
			}

			remaining -= takeCount;
		}

		reversedEntries.reverse();
		return {
			entries: reversedEntries,
			collapsedGroupCount: 0
		};
	});

	let displayedEntries = $derived(displayedEntriesResult.entries);
	let collapsedGroupCount = $derived(displayedEntriesResult.collapsedGroupCount);
	let isWindowedView = $derived(
		followLatest &&
			focusedRecordKey === null &&
			!isFiltered &&
			(collapseDuplicates
				? collapseAllDuplicates
					? collapsedGroupCount > effectiveRenderLimit
					: records.length > effectiveRenderLimit
				: rawEntryLimit > effectiveRenderLimit)
	);

	let filteredEntries = $derived.by<LoggerEntry[]>(() => {
		if (!isFiltered) {
			return displayedEntries;
		}

		return displayedEntries.filter((entry) => {
			if (
				normalizedSourceFilter.length > 0 &&
				!entry.sourceFilterText.includes(normalizedSourceFilter)
			) {
				return false;
			}
			if (normalizedTagFilter.length > 0 && !entry.tagFilterText.includes(normalizedTagFilter)) {
				return false;
			}
			if (
				normalizedContentFilter.length > 0 &&
				!entry.contentFilterText.includes(normalizedContentFilter)
			) {
				return false;
			}
			return true;
		});
	});

	let renderedEntries = $derived(filteredEntries);

	const withSuppressedScroll = (operation: () => void): void => {
		suppressedScrollEvents += 1;
		operation();
		requestAnimationFrame(() => {
			suppressedScrollEvents = Math.max(0, suppressedScrollEvents - 1);
		});
	};

	const nowMs = (): number => {
		if (typeof performance !== 'undefined' && typeof performance.now === 'function') {
			return performance.now();
		}
		return Date.now();
	};

	const markUserScrollIntent = (): void => {
		userScrollIntentUntilMs = nowMs() + 350;
	};

	const hasRecentUserScrollIntent = (): boolean => nowMs() <= userScrollIntentUntilMs;

	const trackUserScrollIntent = (element: HTMLElement) => {
		const onWheel = () => markUserScrollIntent();
		const onPointerDown = () => markUserScrollIntent();
		const onTouchStart = () => markUserScrollIntent();

		element.addEventListener('wheel', onWheel, { passive: true });
		element.addEventListener('pointerdown', onPointerDown);
		element.addEventListener('touchstart', onTouchStart, { passive: true });

		return {
			destroy() {
				element.removeEventListener('wheel', onWheel);
				element.removeEventListener('pointerdown', onPointerDown);
				element.removeEventListener('touchstart', onTouchStart);
			}
		};
	};

	const isListAtBottom = (): boolean => {
		if (!loggerList) {
			return true;
		}
		const maxScrollTop = loggerList.scrollHeight - loggerList.clientHeight;
		if (maxScrollTop <= 0) {
			return true;
		}
		return maxScrollTop - loggerList.scrollTop <= LIST_BOTTOM_EPSILON_PX;
	};

	const scrollToBottom = (): void => {
		if (!loggerList) {
			return;
		}
		withSuppressedScroll(() => {
			if (listEndMarker) {
				listEndMarker.scrollIntoView({ block: 'end' });
			}
			if (loggerList) loggerList.scrollTop = loggerList?.scrollHeight ?? 0;
		});
		requestAnimationFrame(() => {
			if (!loggerList || !followLatest || focusedRecordKey !== null || isListAtBottom()) {
				return;
			}
			withSuppressedScroll(() => {
				if (listEndMarker) {
					listEndMarker.scrollIntoView({ block: 'end' });
				}
				if (loggerList) loggerList.scrollTop = loggerList?.scrollHeight ?? 0;
			});
		});
	};

	const findRecordElement = (entryKey: string): HTMLElement | null => {
		if (!loggerList) {
			return null;
		}
		return loggerList.querySelector<HTMLElement>(`[data-record-key="${entryKey}"]`);
	};

	const scrollFocusedRecordIntoView = (): void => {
		if (!focusedRecordKey) {
			return;
		}
		const focusedElement = findRecordElement(focusedRecordKey);
		if (!focusedElement) {
			return;
		}
		withSuppressedScroll(() => {
			focusedElement.scrollIntoView({ block: 'nearest' });
		});
	};

	const syncViewport = (): void => {
		if (!loggerList) {
			return;
		}

		if (focusedRecordKey) {
			const focusedRecordStillVisible = filteredEntries.some(
				(entry) => entry.key === focusedRecordKey
			);
			if (!focusedRecordStillVisible) {
				focusedRecordKey = null;
				followLatest = true;
				scrollToBottom();
				return;
			}

			scrollFocusedRecordIntoView();
			return;
		}

		if (followLatest) {
			scrollToBottom();
		}
	};

	const scheduleViewportSync = (): void => {
		if (viewportSyncRaf !== null) {
			cancelAnimationFrame(viewportSyncRaf);
		}
		viewportSyncRaf = requestAnimationFrame(() => {
			viewportSyncRaf = null;
			syncViewport();
		});
	};

	$effect(() => {
		if (maxEntries > 0) {
			desiredMaxEntries = maxEntries;
		}
	});

	$effect(() => {
		const normalized = normalizeLogUiUpdateHz(logUiUpdateHz);
		desiredLogUiUpdateHz = normalized;
	});

	$effect(() => {
		displayedEntries;
		filteredEntries;
		focusedRecordKey;
		followLatest;
		scheduleViewportSync();
	});

	const selectOrigin = (event: MouseEvent, origin: NodeId | undefined, entryKey: string): void => {
		event.stopPropagation();
		focusedRecordKey = entryKey;
		followLatest = false;
		if (origin !== undefined) {
			session?.selectNode(origin);
		}
		scheduleViewportSync();
	};

	const focusRecord = (entryKey: string): void => {
		focusedRecordKey = entryKey;
		followLatest = false;
		scheduleViewportSync();
	};

	const handleRecordClick = (entryKey: string): void => {
		if (hasLoggerSelection()) {
			return;
		}
		focusRecord(entryKey);
	};

	const copyEntry = (event: MouseEvent, entry: LoggerEntry): void => {
		event.stopPropagation();
		void copyTextToClipboard(formatLogEntryForClipboard(entry));
	};

	const copyDisplayedLogs = (): void => {
		if (filteredEntries.length === 0) {
			return;
		}

		const text = filteredEntries.map((entry) => formatLogEntryForClipboard(entry)).join('\n\n');
		void copyTextToClipboard(text);
	};

	const handleRecordKeydown = (event: KeyboardEvent, entryKey: string): void => {
		if (event.key !== 'Enter' && event.key !== ' ') {
			return;
		}
		event.preventDefault();
		focusRecord(entryKey);
	};

	const resyncToLatest = (): void => {
		focusedRecordKey = null;
		followLatest = true;
		scheduleViewportSync();
	};

	const clearFilters = (): void => {
		sourceFilter = '';
		tagFilter = '';
		contentFilter = '';
	};

	const toggleCollapse = (): void => {
		collapseDuplicates = !collapseDuplicates;
	};

	const toggleSuperCollapse = (): void => {
		collapseAllDuplicates = !collapseAllDuplicates;
	};

	const applyMaxEntries = (): void => {
		const normalized = Math.max(1, Math.round(Number(desiredMaxEntries)));
		if (!Number.isFinite(normalized)) {
			desiredMaxEntries = maxEntries > 0 ? maxEntries : 1;
			return;
		}
		desiredMaxEntries = normalized;
		void sendSetLogMaxEntriesIntent(normalized);
	};

	const handleMaxEntriesKeydown = (event: KeyboardEvent): void => {
		if (event.key !== 'Enter') {
			return;
		}
		event.preventDefault();
		applyMaxEntries();
	};

	const applyLogUiUpdateHz = (): void => {
		const normalized = normalizeLogUiUpdateHz(Number(desiredLogUiUpdateHz));
		desiredLogUiUpdateHz = normalized;
		session?.setLogUiUpdateHz(normalized);
	};

	const handleLogUiUpdateHzKeydown = (event: KeyboardEvent): void => {
		if (event.key !== 'Enter') {
			return;
		}
		event.preventDefault();
		applyLogUiUpdateHz();
	};

	const clearLogs = (): void => {
		focusedRecordKey = null;
		followLatest = true;
		void sendClearLogsIntent();
	};

	const handleListScroll = (): void => {
		if (suppressedScrollEvents > 0) {
			return;
		}

		if (!isListAtBottom()) {
			if (followLatest && focusedRecordKey === null && !hasRecentUserScrollIntent()) {
				scheduleViewportSync();
				return;
			}
			if (focusedRecordKey === null) {
				followLatest = false;
			}
			return;
		}

		focusedRecordKey = null;
		followLatest = true;
	};

	onMount(() => {
		scheduleViewportSync();
		return () => {
			if (viewportSyncRaf !== null) {
				cancelAnimationFrame(viewportSyncRaf);
			}
		};
	});
</script>

<section class="logger-panel">
	<header class="logger-toolbar">
		<!-- <div class={`logger-stats${isFiltered ? ' is-filtered' : ''}`}>
			<strong>{filteredEntries.length}</strong>
			<span>/ {displayedEntries.length}</span>
			{#if collapseDuplicates}
				<span class="raw-count">({records.length})</span>
			{/if}
		</div> -->

		<!-- <label class="max-input">
			<span>max</span>
			<input
				type="number"
				min="1"
				step="1"
				bind:value={desiredMaxEntries}
				onchange={applyMaxEntries}
				onkeydown={handleMaxEntriesKeydown} />
		</label>
		<label class="hz-input" title="Logger UI update rate limit">
			<span>hz</span>
			<input
				type="number"
				min={MIN_LOG_UI_UPDATE_HZ}
				max={MAX_LOG_UI_UPDATE_HZ}
				step="1"
				bind:value={desiredLogUiUpdateHz}
				onchange={applyLogUiUpdateHz}
				onkeydown={handleLogUiUpdateHzKeydown} />
		</label> -->

		<input
			class="filter-input source"
			type="search"
			placeholder="source"
			bind:value={sourceFilter} />
		<input class="filter-input tag" type="search" placeholder="tag" bind:value={tagFilter} />
		<input
			class="filter-input content"
			type="search"
			placeholder="content"
			bind:value={contentFilter} />
		<div class="spacer"></div>
		<button type="button" class="toolbar-button" onclick={clearFilters} disabled={!isFiltered}>
			Clear filters
		</button>
	</header>

	<div
		class={`logger-list${isFiltered ? ' is-filtered' : ''}${isWindowedView ? ' is-windowed' : ''}`}
		bind:this={loggerList}
		use:trackUserScrollIntent
		onscroll={handleListScroll}>
		{#if filteredEntries.length === 0}
			<div class="empty-state">
				{#if records.length === 0}
					No logs yet.
				{:else if isFiltered}
					No log matches the current filters.
				{:else}
					No logs yet.
				{/if}
			</div>
		{:else}
			{#each renderedEntries as entry (entry.key)}
				{@const record = entry.record}
				<div
					class={`record level-${record.level}${focusedRecordKey === entry.key ? ' is-focused' : ''}`}
					role="button"
					tabindex="0"
					data-record-key={entry.key}
					onclick={() => handleRecordClick(entry.key)}
					onkeydown={(event) => handleRecordKeydown(event, entry.key)}
					onmouseenter={(e: MouseEvent) => {
						overRecordKey = entry.key;
						copyButtonText = null;
					}}
					onmouseleave={() => (overRecordKey = null)}>
					<time class="time">{entry.formattedTime}</time>
					<!-- <span class="level">{record.level}</span> -->
					<!-- <span class="tag">{record.tag}</span> -->

					<p class="message">{record.message}</p>

					<div class="record-actions">
						{#if record.origin !== undefined}
							<button
								type="button"
								class="origin"
								onclick={(event) => selectOrigin(event, record.origin, entry.key)}
								title="Select source node">
								{entry.sourceLabel}
							</button>
						{:else}
							<span class="origin-label">{entry.sourceLabel}</span>
						{/if}

						{#if entry.repeatCount > 1}
							<span class="repeat-count" title={`${entry.repeatCount} repeated logs`}>
								x{entry.repeatCount}
							</span>
						{/if}

						{#if overRecordKey === entry.key}
							<button
								transition:fade={{ duration: 150 }}
								type="button"
								class="copy-log-button"
								title="Copy this log entry"
								aria-label="Copy this log entry"
								onclick={(event) => {
									copyEntry(event, entry);
									copyButtonText = 'Copied !';
								}}
								onmouseleave={() => (copyButtonText = null)}>
								{#if copyButtonText}
									<div class="copy-text" transition:slide={{ duration: 200, axis: 'x' }}>
										{copyButtonText}
									</div>
								{/if}
							</button>
						{/if}
					</div>
				</div>
			{/each}
		{/if}
		<div class="list-end-marker" bind:this={listEndMarker} aria-hidden="true"></div>
	</div>
	<footer class="logger-footer">
		<button
			type="button"
			class={`toolbar-button collapse${collapseDuplicates ? ' is-active' : ''}`}
			onclick={toggleCollapse}>
			Collapse
		</button>
		<button
			type="button"
			class={`toolbar-button super-collapse${collapseAllDuplicates ? ' is-active' : ''}`}
			title="Aggregate all matching messages, not just consecutive ones."
			onclick={toggleSuperCollapse}
			disabled={!collapseDuplicates}>
			All duplicates
		</button>

		<button
			type="button"
			class={`toolbar-button live${followLatest && focusedRecordKey === null ? '' : ' is-active'}`}
			onclick={resyncToLatest}
			disabled={followLatest && focusedRecordKey === null}>
			Live
		</button>
		<!-- {#if isWindowedView}
			<span class="windowed-indicator" title="Live mode renders only recent logs for performance">
				tail {effectiveRenderLimit}
			</span>
		{/if} -->
		<div class="spacer"></div>
		<button
			type="button"
			class="toolbar-button"
			title="Copy the logs currently displayed in this panel"
			onclick={copyDisplayedLogs}
			disabled={filteredEntries.length === 0}>
			Copy logs
		</button>
		<button type="button" class="toolbar-button clear-logs" onclick={clearLogs}>Clear logs</button>
	</footer>
</section>

<style>
	.logger-panel {
		display: flex;
		flex-direction: column;
		block-size: 100%;
		min-block-size: 0;
		gap: 0.25rem;
	}

	.logger-toolbar {
		display: flex;
		align-items: center;
		gap: 0.3rem;
		border-bottom: 0.0625rem solid
			color-mix(in srgb, var(--gc-color-panel-outline) 85%, transparent);
		background: color-mix(in srgb, var(--gc-color-panel-row) 70%, black);
	}

	header input {
		font-size: 0.6rem;
		text-transform: uppercase;
		width: 5rem;
	}

	footer {
		display: flex;
	}

	.toolbar-button {
		block-size: 1.7rem;
		padding: 0.1rem 0.45rem;
		border: 0.0625rem solid color-mix(in srgb, var(--gc-color-panel-outline) 80%, transparent);
		border-radius: 0.25rem;
		background: color-mix(in srgb, var(--gc-color-panel-row) 82%, black);
		color: inherit;
		font-size: 0.66rem;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		cursor: pointer;
		user-select: none;
		-webkit-user-select: none;
	}

	.toolbar-button:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.toolbar-button.collapse.is-active,
	.toolbar-button.super-collapse.is-active,
	.toolbar-button.live.is-active {
		border-color: color-mix(in srgb, var(--gc-color-focus) 60%, transparent);
		background: color-mix(in srgb, var(--gc-color-focus-muted) 70%, black);
	}

	.toolbar-button.clear-logs {
		border-color: color-mix(in srgb, #bf4d43 55%, transparent);
	}

	.windowed-indicator {
		padding: 0 0.32rem;
		border: 0.0625rem solid color-mix(in srgb, #cf9b37 45%, transparent);
		border-radius: 0.25rem;
		background: color-mix(in srgb, #cf9b37 22%, transparent);
		font-size: 0.64rem;
		line-height: 1.6;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		white-space: nowrap;
	}

	.logger-list {
		flex: 1;
		min-block-size: 0;
		overflow: auto;
		padding: 0.18rem 0.35rem 0.4rem;
		display: flex;
		flex-direction: column;
		font-family: ui-monospace, 'SFMono-Regular', Menlo, Consolas, 'Liberation Mono', monospace;
		font-variant-ligatures: none;
		user-select: text;
		-webkit-user-select: text;
	}

	.logger-list.is-filtered {
		box-shadow: inset 0 0 0 0.0625rem color-mix(in srgb, var(--gc-color-focus) 45%, transparent);
		background: color-mix(in srgb, var(--gc-color-focus-muted) 30%, transparent);
	}

	.logger-list.is-windowed {
		outline: 0.0625rem dashed color-mix(in srgb, #cf9b37 40%, transparent);
		outline-offset: -0.0625rem;
	}

	.list-end-marker {
		inline-size: 100%;
		block-size: 0.0625rem;
		flex: 0 0 auto;
		pointer-events: none;
	}

	.empty-state {
		padding: 0.6rem 0.4rem;
		opacity: 0.7;
		font-size: 0.76rem;
	}

	.record {
		display: flex;
		align-items: flex-start;
		gap: 0.35rem;
		padding: 0.16rem 0.24rem;
		border-radius: 0.22rem;
		border-bottom: 0.0625rem solid
			color-mix(in srgb, var(--gc-color-panel-outline) 65%, transparent);
		contain: layout style paint;
		content-visibility: auto;
		contain-intrinsic-size: 1.4rem;
		user-select: text;
		-webkit-user-select: text;
	}

	.record:hover {
		background: color-mix(in srgb, var(--gc-color-panel-row) 65%, black);
	}

	.record:focus-visible {
		outline: 0.0625rem solid color-mix(in srgb, var(--gc-color-focus) 65%, transparent);
	}

	.record.is-focused {
		outline: 0.0625rem solid color-mix(in srgb, var(--gc-color-focus) 70%, transparent);
		background: color-mix(in srgb, var(--gc-color-focus-muted) 45%, black);
	}

	.record {
		.record-actions {
			display: flex;
			align-items: flex-start;
			gap: 0.35rem;
			flex: 0 0 auto;
			min-inline-size: 0;
		}

		.time,
		.level,
		.tag,
		.repeat-count,
		.copy-log-button,
		.origin,
		.origin-label {
			flex: 0 0 auto;
			font-size: 0.66rem;
			line-height: 1.35;
		}

		.time {
			min-inline-size: 7.6em;
			opacity: 0.72;
		}

		.level {
			min-inline-size: 4.8em;
			text-transform: uppercase;
			font-weight: 650;
		}

		.tag,
		.repeat-count {
			padding: 0 0.22rem;
			border-radius: 0.22rem;
			background: color-mix(in srgb, var(--gc-color-panel-alt) 84%, black);
			white-space: nowrap;
		}

		.tag {
			max-inline-size: 14em;
			overflow: hidden;
			text-overflow: ellipsis;
		}

		.repeat-count {
			color: color-mix(in srgb, var(--gc-color-text) 85%, white 15%);
		}

		.origin,
		.origin-label {
			max-inline-size: 18em;
			overflow: hidden;
			text-overflow: ellipsis;
			white-space: nowrap;
			text-align: start;
		}

		.origin {
			padding: 0;
			border: 0;
			background: transparent;
			color: color-mix(in srgb, var(--gc-color-text) 80%, white 20%);
			cursor: pointer;
		}

		.origin:hover {
			color: color-mix(in srgb, var(--gc-color-focus) 80%, white 20%);
		}

		.origin-label {
			opacity: 0.78;
		}

		.copy-log-button {
			border: 0;
			color: color-mix(in srgb, var(--gc-color-panel-outline) 60%, white 40%);
			cursor: pointer;
			user-select: none;
			-webkit-user-select: none;
			min-inline-size: 1rem;
			block-size: 1rem;
			background-color: var(--gc-color-background);
			background-image: url('../../../style/icons/clipboard.svg');
			background-repeat: no-repeat;
			background-size: contain;
			background-position: center right;
			border-radius: 1rem;
			padding: 0;
		}

		.copy-log-button:hover {
			background-color: color-mix(
				in srgb,
				var(--gc-color-focus) 10%,
				var(--gc-color-background) 90%
			);
		}

		.copy-log-button .copy-text {
			padding: 0rem 1.5rem 0rem 1rem;
		}

		.message {
			margin: 0;
			flex: 1 1 auto;
			min-inline-size: 0;
			font-size: 0.72rem;
			line-height: 1.35;
			overflow-wrap: anywhere;
			word-break: break-word;
			white-space: pre-wrap;
		}
	}

	.level-info .message {
		color: var(--gc-color-text);
	}

	.level-success .message {
		color: var(--gc-color-success);
	}

	.level-warning .message {
		color: var(--gc-color-warning);
	}

	.level-error .message {
		color: var(--gc-color-error);
	}
</style>
