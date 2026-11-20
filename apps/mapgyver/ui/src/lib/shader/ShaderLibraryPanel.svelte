<script lang="ts">
	import { FileExplorerPanel, IconButton, type PanelProps, type PanelState } from 'golden_ui';
	import { hasDesktopHost } from 'golden_ui/host/desktop';
	import shaderIcon from 'golden_ui/style/icons/node/dashboard.svg';
	import refreshIcon from 'golden_ui/style/icons/reset.svg';
	import { SHADER_LIBRARY_SUFFIXES } from '../media/generated/constants';
	import type { ShaderLibraryEntry } from '../media/generated/ShaderLibraryEntry';
	import {
		beginShaderDrag,
		browseFragCoord,
		browseIsf,
		browseShaderToy,
		endShaderDrag,
		setShaderToyKey,
		shaderToyKeyConfigured,
		type OnlineShaderSource
	} from './library';

	let props: PanelProps = $props();
	let explorer = $state<{ setPanelState(state: PanelState): void }>();
	export const setPanelState = (state: PanelState) => explorer?.setPanelState(state);
	const desktop = hasDesktopHost();
	let source = $state<'local' | OnlineShaderSource>('local');
	let query = $state('');
	let isfEntries = $state.raw<ShaderLibraryEntry[]>([]);
	let isfRequested = false;
	let isfPending = $state(false);
	let isfError = $state('');
	let remoteEntries = $state.raw<ShaderLibraryEntry[]>([]);
	let remotePending = $state(false);
	let remoteError = $state('');
	let offset = $state(0);
	let nextOffset = $state<number | null>(null);
	let revision = $state(0);
	let keyConfigured = $state<boolean | null>(null);
	let editingKey = $state(false);
	let storingKey = $state(false);
	let keyError = $state('');
	const shaderToy = $derived(source === 'shadertoy');
	const paged = $derived(source === 'fragcoord' || shaderToy);
	const libraryReady = $derived(
		source !== 'local' && (!shaderToy || (keyConfigured && !editingKey))
	);
	const catalogName = $derived(
		shaderToy ? 'ShaderToy' : source === 'fragcoord' ? 'FragCoord' : 'ISF'
	);
	const pending = $derived(paged ? remotePending : isfPending);
	const error = $derived(paged ? remoteError : isfError);
	const matches = $derived(
		paged
			? remoteEntries
			: isfEntries.filter(({ label }) => label.toLowerCase().includes(query.trim().toLowerCase()))
	);

	async function loadIsf(refresh: boolean) {
		isfRequested = true;
		isfPending = true;
		isfError = '';
		try {
			isfEntries = await browseIsf(refresh);
		} catch (cause) {
			isfError = cause instanceof Error ? cause.message : String(cause);
		} finally {
			isfPending = false;
		}
	}

	$effect(() => {
		if (desktop && source === 'isf' && !isfRequested) void loadIsf(false);
	});

	$effect(() => {
		if (!desktop || !shaderToy || keyConfigured !== null) return;
		let current = true;
		void shaderToyKeyConfigured().then(
			(configured) => {
				if (current) keyConfigured = configured;
			},
			(cause) => {
				if (!current) return;
				keyError = cause instanceof Error ? cause.message : String(cause);
				keyConfigured = false;
			}
		);
		return () => {
			current = false;
		};
	});

	$effect(() => {
		if (!desktop || !paged || !libraryReady) return;
		const browse = shaderToy ? browseShaderToy : browseFragCoord;
		const search = query.trim();
		const start = offset;
		const request = revision;
		let current = true;
		if (start === 0) remoteEntries = [];
		remotePending = true;
		remoteError = '';
		nextOffset = null;
		const timer = setTimeout(
			async () => {
				try {
					const page = await browse(search, start);
					if (!current || revision !== request) return;
					const seen = new Set(remoteEntries.map(({ id }) => id));
					remoteEntries = [...remoteEntries, ...page.entries.filter(({ id }) => !seen.has(id))];
					nextOffset = page.next_offset;
				} catch (cause) {
					if (current) remoteError = cause instanceof Error ? cause.message : String(cause);
				} finally {
					if (current) remotePending = false;
				}
			},
			start === 0 ? 220 : 0
		);
		return () => {
			current = false;
			clearTimeout(timer);
		};
	});

	function selectSource(event: Event & { currentTarget: HTMLSelectElement }) {
		source = event.currentTarget.value as typeof source;
		query = '';
		offset = 0;
		editingKey = false;
		if (source === 'shadertoy') keyConfigured = null;
	}

	function refresh() {
		if (paged) {
			offset = 0;
			revision += 1;
		} else void loadIsf(true);
	}

	async function connect(event: SubmitEvent & { currentTarget: HTMLFormElement }) {
		event.preventDefault();
		const form = event.currentTarget;
		const key = new FormData(form).get('key');
		if (typeof key !== 'string' || !key.trim() || storingKey) return;
		storingKey = true;
		keyError = '';
		try {
			await setShaderToyKey(key);
			form.reset();
			keyConfigured = true;
			editingKey = false;
			offset = 0;
			revision += 1;
		} catch (cause) {
			keyError = cause instanceof Error ? cause.message : String(cause);
		} finally {
			storingKey = false;
		}
	}
</script>

<section class="shader-library" aria-label="Shader Library">
	<div class="toolbar">
		<select aria-label="Shader library source" value={source} onchange={selectSource}>
			<option value="local">Local</option>
			<option value="isf" disabled={!desktop}>ISF (VIDVOX)</option>
			<option value="fragcoord" disabled={!desktop}>FragCoord.XYZ</option>
			<option value="shadertoy" disabled={!desktop}>ShaderToy</option>
		</select>
		{#if libraryReady}
			<input
				type="search"
				aria-label="Search {catalogName} shaders"
				placeholder="Search shaders…"
				maxlength="100"
				oninput={() => (offset = 0)}
				bind:value={query} />
			<IconButton
				icon={refreshIcon}
				label="Refresh {catalogName} catalog"
				onclick={refresh}
				disabled={pending} />
			{#if shaderToy}
				<button type="button" class="more" onclick={() => (editingKey = true)}>Change key</button>
			{/if}
		{/if}
	</div>
	<div class="local" hidden={source !== 'local'}>
		<FileExplorerPanel
			{...props}
			title="Local Shaders"
			fileSuffixes={SHADER_LIBRARY_SUFFIXES}
			bind:this={explorer} />
	</div>
	{#if shaderToy && !libraryReady && keyConfigured !== null}
		<form class="key-form" autocomplete="off" onsubmit={connect}>
			<label for="{props.panelId}-shadertoy-key">ShaderToy app key</label>
			<div class="key-actions">
				<input
					id="{props.panelId}-shadertoy-key"
					name="key"
					type="password"
					maxlength="128"
					required />
				<button type="submit" class="more" disabled={storingKey}>Connect</button>
				{#if editingKey}<button type="button" class="more" onclick={() => (editingKey = false)}
						>Cancel</button
					>{/if}
			</div>
			{#if keyError}<div role="status">{keyError}</div>{/if}
		</form>
	{/if}
	{#if shaderToy}<div class="provider">Shadertoy.com API</div>{/if}
	{#if libraryReady}
		{#if pending}<div class="status" role="status">Loading {catalogName} catalog…</div>{/if}
		{#if error}<div class="status" role="status">{error}</div>{/if}
		{#if !pending && !error && !matches.length}<div class="status">No shaders found</div>{/if}
		<ul class="entries" aria-label="{catalogName} catalog">
			{#each matches as entry (entry.id)}
				<li
					draggable="true"
					ondragstart={(event) =>
						source !== 'local' && beginShaderDrag(event, { source, id: entry.id })}
					ondragend={endShaderDrag}
					title="Drag into Medias to import {entry.label}">
					<img src={shaderIcon} alt="" draggable="false" />
					{entry.label}
				</li>
			{/each}
			{#if paged && nextOffset !== null}
				<li class="more-row">
					<button type="button" class="more" onclick={() => (offset = nextOffset ?? offset)}
						>Load more</button>
				</li>
			{/if}
		</ul>
	{/if}
</section>

<style>
	.shader-library {
		display: flex;
		flex-direction: column;
		block-size: 100%;
		min-block-size: 0;
		min-inline-size: 0;
	}
	.toolbar {
		display: flex;
		align-items: center;
		gap: 0.3rem;
		padding: 0.2rem 0.35rem;
	}
	select,
	input {
		min-inline-size: 0;
		padding: 0.2rem 0.35rem;
		border: 0.06rem solid var(--gc-color-border, #ffffff28);
		border-radius: 0.25rem;
		background: var(--gc-color-bg-input, #ffffff0d);
		color: inherit;
		font: inherit;
	}
	input {
		flex: 1;
	}
	.key-form {
		display: grid;
		gap: 0.3rem;
		padding: 0.35rem 0.55rem;
	}
	.key-actions {
		display: flex;
		gap: 0.3rem;
	}
	.provider {
		padding: 0.2rem 0.55rem;
		font-size: 0.75rem;
	}
	.local,
	.entries {
		flex: 1;
		min-block-size: 0;
	}
	.entries {
		overflow: auto;
		list-style: none;
		margin: 0;
		padding: 0.2rem 0.4rem;
	}
	.entries li {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.22rem 0.35rem;
		border-radius: 0.25rem;
		cursor: grab;
		user-select: none;
	}
	.entries li:hover {
		background: var(--gc-control-hover-background, #ffffff18);
	}
	.entries .more-row {
		cursor: default;
		background: transparent;
	}
	.more {
		padding: 0.2rem 0.35rem;
		border: 0;
		border-radius: 0.25rem;
		background: transparent;
		color: inherit;
		font: inherit;
		cursor: pointer;
	}
	.more:hover {
		background: var(--gc-control-hover-background, #ffffff18);
	}
	.entries img {
		inline-size: 0.9rem;
		block-size: 0.9rem;
	}
	.status {
		padding: 0.35rem 0.55rem;
	}
</style>
