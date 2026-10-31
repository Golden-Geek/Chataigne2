<script lang="ts">
	import { untrack } from 'svelte';
	import { appState } from 'golden_ui';
	import type { ChannelSnapshot } from './generated/ChannelSnapshot';
	import type { MediaManagerSnapshot } from './generated/MediaManagerSnapshot';
	import type { MatrixSnapshot } from './generated/MatrixSnapshot';
	import type { MatrixEdit } from './generated/MatrixEdit';
	import type { MediaEdit } from './generated/MediaEdit';
	import type { MediaEditResult } from './generated/MediaEditResult';
	import type { TransitionCurve } from './generated/TransitionCurve';
	import { MEDIA_NAMESPACE, MATRIX_PLAYBACK_EVENT } from './generated/constants';
	import type { MatrixPlaybackSnapshot } from './generated/MatrixPlaybackSnapshot';
	import { invokeMediaEdit } from './operations';
	import { channelKey, channelReferenceFromKey } from './identity';
	import SourcePicker from '../composition/SourcePicker.svelte';
	let {
		channel,
		catalog,
		disabled,
		edit
	}: {
		channel: ChannelSnapshot;
		catalog: MediaManagerSnapshot;
		disabled: boolean;
		edit(intent: MediaEdit): Promise<MediaEditResult | undefined>;
	} = $props();
	const session = $derived(appState.session);
	let snapshot = $state<MatrixSnapshot | null>(null);
	let playback = $state<MatrixPlaybackSnapshot | null>(null);
	const columnLabels = $derived(
		new Map(snapshot?.columns.map((column) => [column.id, column.label]))
	);
	let playbackRequest = 0;
	async function refreshPlayback(current = session, media = channel.reference.media) {
		const request = ++playbackRequest,
			generation = current?.snapshotGeneration;
		playback = null;
		if (!current || current.status !== 'connected' || current.loading.visible) return;
		try {
			const result = await invokeMediaEdit(current, {
				kind: 'Matrix',
				media,
				edit: { kind: 'InspectPlayback' }
			});
			if (
				request === playbackRequest &&
				session === current &&
				current.status === 'connected' &&
				!current.loading.visible &&
				current.snapshotGeneration === generation &&
				channel.reference.media === media &&
				result.kind === 'MatrixPlayback'
			)
				playback = result.snapshot;
		} catch (cause) {
			if (
				request === playbackRequest &&
				session === current &&
				current.snapshotGeneration === generation &&
				channel.reference.media === media
			)
				error = String(cause);
		}
	}
	$effect(() => {
		const current = session,
			media = channel.reference.media;
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.getCustomEventSequence(MATRIX_PLAYBACK_EVENT);
		}
		untrack(() => {
			void refreshPlayback(current, media);
		});
		return () => {
			++playbackRequest;
		};
	});
	let refreshing = $state(false),
		pending = $state(false),
		error = $state('');
	let width = $state<number | undefined>(),
		height = $state<number | undefined>();
	let duration = $state<number | undefined>(),
		curve = $state<TransitionCurve | undefined>();
	let layerSearch = $state(''),
		columnSearch = $state('');
	const layers = $derived(snapshot?.items.filter((item) => item.id.kind === 'Layer') ?? []);
	const itemLabels = $derived.by(() => {
		const names = new Map<string, string>();
		for (const item of snapshot?.items ?? [])
			names.set(item.id.id, item.parent ? `${names.get(item.parent)} / ${item.label}` : item.label);
		return names;
	});
	const authoredCells = $derived.by(() => {
		const cells = snapshot?.cells.slice(0, 20) ?? [];
		const columns = new Map(snapshot?.columns.map((column) => [column.id, column.label]));
		const wantedMedia = new Set(
			cells.flatMap((cell) => (cell.definition.source ? [cell.definition.source.media] : []))
		);
		const wantedSources = new Set(
			cells.flatMap((cell) => (cell.definition.source ? [channelKey(cell.definition.source)] : []))
		);
		const sources = new Map<string, string>();
		for (const media of catalog.media)
			if (wantedMedia.has(media.id))
				for (const channel of media.channels) {
					const key = channelKey(channel.reference);
					if (wantedSources.has(key)) sources.set(key, `${media.label} / ${channel.label}`);
				}
		return cells.map((cell) => ({
			id: cell.id,
			layer: itemLabels.get(cell.layer),
			column: columns.get(cell.definition.column),
			source: cell.definition.source
				? (sources.get(channelKey(cell.definition.source)) ?? 'Missing source')
				: 'Empty'
		}));
	});
	const visibleLayers = $derived(
		layers
			.filter((item) => item.label.toLocaleLowerCase().includes(layerSearch.toLocaleLowerCase()))
			.slice(0, 100)
	);
	const visibleColumns = $derived(
		snapshot?.columns
			.filter((column) =>
				column.label.toLocaleLowerCase().includes(columnSearch.toLocaleLowerCase())
			)
			.slice(0, 100) ?? []
	);
	const busy = $derived(disabled || pending || refreshing || session?.status !== 'connected');
	let request = 0;
	let context: { session: typeof session; generation: number; media: string } | null = null;
	async function refresh(current = session, media = channel.reference.media) {
		const read = ++request;
		if (!current || current.status !== 'connected' || current.loading.visible) {
			snapshot = null;
			refreshing = false;
			return;
		}
		const generation = current.snapshotGeneration;
		const valid = () =>
			read === request &&
			session === current &&
			current.status === 'connected' &&
			!current.loading.visible &&
			current.snapshotGeneration === generation &&
			channel.reference.media === media;
		if (
			context?.session !== current ||
			context.generation !== generation ||
			context.media !== media
		)
			snapshot = null;
		context = { session: current, generation, media };
		if (current.hasActiveEditSession) {
			refreshing = false;
			return;
		}
		refreshing = true;
		try {
			const result = await invokeMediaEdit(current, {
				kind: 'Matrix',
				media,
				edit: { kind: 'Inspect' }
			});
			if (!valid()) return;
			if (result.kind !== 'Matrix') throw new Error('Unexpected Matrix inspection result');
			snapshot = result.snapshot;
			error = '';
		} catch (cause) {
			if (valid()) error = String(cause);
		} finally {
			if (read === request) refreshing = false;
		}
	}
	$effect(() => {
		const current = session,
			media = channel.reference.media;
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.currentHistoryStateId;
			void current.hasActiveEditSession;
			void current.editSessionEpoch;
			void current.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		untrack(() => {
			void refresh(current, media);
		});
		return () => {
			++request;
		};
	});
	$effect(() => {
		width = snapshot?.definition.canvas.width;
		height = snapshot?.definition.canvas.height;
		duration = snapshot?.definition.transition.duration_ms;
		curve = snapshot?.definition.transition.curve;
	});
	async function mutate(intent: MatrixEdit) {
		if (busy) return;
		pending = true;
		error = '';
		const current = session,
			generation = current?.snapshotGeneration,
			media = channel.reference.media;
		try {
			const result = await edit({ kind: 'Matrix', media, edit: intent });
			if (
				result &&
				session === current &&
				current?.snapshotGeneration === generation &&
				channel.reference.media === media
			)
				if (result.kind === 'MatrixLaunched') await refreshPlayback(current, media);
				else await refresh(current, media);
		} finally {
			pending = false;
		}
	}
</script>

<section aria-label="Matrix document">
	<header>
		<h3>Matrix</h3>
		<button disabled={busy} onclick={() => refresh()}>Refresh Matrix</button>
	</header>
	{#if error}<p role="alert">{error}</p>{/if}
	{#if snapshot}
		<p>
			{layers.length} layers · {snapshot.items.length - layers.length} groups · {snapshot.columns
				.length} columns · {snapshot.cells.length} authored cells
		</p>
		<div class="actions">
			<button disabled={busy} onclick={() => mutate({ kind: 'AddLayers', parent: null, count: 1 })}
				>Add layer</button>
			<button
				disabled={busy}
				onclick={() => mutate({ kind: 'AddGroup', parent: null, label: null })}>Add group</button>
			<button disabled={busy} onclick={() => mutate({ kind: 'AddColumns', count: 1 })}
				>Add column</button>
		</div>
		<fieldset disabled={busy}>
			<legend>Column playback</legend>
			<p>
				Requested column: {playback?.desired_column
					? (columnLabels.get(playback.desired_column) ?? 'Missing column')
					: 'None'}
			</p>
			<p>Launch uses the column transition override or Matrix default. Cut switches immediately.</p>
			<input
				type="search"
				aria-label="Find column to launch"
				placeholder="Find a column…"
				bind:value={columnSearch} />
			<div class="actions">
				{#each visibleColumns as column (column.id)}
					<button
						type="button"
						onclick={() => mutate({ kind: 'LaunchColumn', column: column.id, mode: 'Configured' })}
						>Launch {column.label}</button>
					<button
						type="button"
						onclick={() => mutate({ kind: 'LaunchColumn', column: column.id, mode: 'Cut' })}
						>Cut to {column.label}</button>
				{/each}
			</div>
			<small
				>Shows up to 100 matching columns. Requested state is distinct from renderer installation.</small>
		</fieldset>
		<form
			onsubmit={(event) => {
				event.preventDefault();
				if (
					snapshot &&
					width !== undefined &&
					height !== undefined &&
					duration !== undefined &&
					curve
				)
					void mutate({
						kind: 'Configure',
						definition: {
							...snapshot.definition,
							canvas: { width, height },
							transition: { duration_ms: duration, curve }
						}
					});
			}}>
			<fieldset disabled={busy}>
				<legend>Canvas and transition</legend>
				<label
					>Canvas width<input type="number" min="1" step="1" required bind:value={width} /></label>
				<label
					>Canvas height<input
						type="number"
						min="1"
						step="1"
						required
						bind:value={height} /></label>
				<label
					>Transition duration (ms)<input
						type="number"
						min="0"
						step="1"
						required
						bind:value={duration} /></label>
				<label
					>Curve<select bind:value={curve}
						><option value="Linear">Linear</option><option value="SmoothStep">Smooth step</option
						></select
					></label>
				<p>Empty cell policy: Transparent</p>
				<button>Apply Matrix settings</button>
			</fieldset>
		</form>
		{#if layers.length && snapshot.columns.length}
			<form
				onsubmit={(event) => {
					event.preventDefault();
					const values = new FormData(event.currentTarget),
						source = String(values.get('source'));
					void mutate({
						kind: 'SetCells',
						assignments: [
							{
								layer: String(values.get('layer')),
								column: String(values.get('column')),
								source: source ? channelReferenceFromKey(source) : null
							}
						]
					});
				}}>
				<fieldset disabled={busy}>
					<legend>Assign a cell source</legend>
					<input
						type="search"
						aria-label="Find Matrix layer"
						placeholder="Find a layer…"
						bind:value={layerSearch} />
					<label
						>Layer
						<select name="layer" required>
							<option value="" disabled selected>Choose a layer</option>
							{#each visibleLayers as item (item.channel.channel)}
								{#if item.id.kind === 'Layer'}
									<option value={item.id.id}>{itemLabels.get(item.id.id)}</option>
								{/if}
							{/each}
						</select>
					</label>
					<input
						type="search"
						aria-label="Find Matrix column"
						placeholder="Find a column…"
						bind:value={columnSearch} />
					<label
						>Column
						<select name="column" required>
							<option value="" disabled selected>Choose a column</option>
							{#each visibleColumns as column (column.id)}
								<option value={column.id}>{column.label}</option>
							{/each}
						</select>
					</label>
					<small>Selectors show up to 100 matches; narrow the search to find more.</small>
					<SourcePicker {catalog} emptyLabel="Clear source" /><button>Assign source</button>
				</fieldset>
			</form>
		{/if}
		{#if authoredCells.length}
			<ul aria-label="Authored cell assignments">
				{#each authoredCells as cell (cell.id)}
					<li>{cell.layer} / {cell.column}: {cell.source}</li>
				{/each}
			</ul>
			{#if snapshot.cells.length > 20}<small>Showing the first 20 authored cells.</small>{/if}
		{/if}
	{:else}<p>Loading Matrix document…</p>{/if}
</section>

<style>
	section,
	fieldset,
	label {
		display: grid;
		gap: 0.6rem;
		min-width: 0;
	}
	header,
	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem;
	}
	h3,
	p {
		margin: 0;
	}
	fieldset {
		border: 0.0625rem solid #444b57;
		border-radius: 0.4rem;
		padding: 0.75rem;
	}
	input,
	select {
		min-width: 0;
		width: 100%;
	}
</style>
