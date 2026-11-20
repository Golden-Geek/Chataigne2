<script lang="ts">
	import { FileExplorerPanel, IconButton, type PanelProps, type PanelState } from 'golden_ui';
	import { hasDesktopHost } from 'golden_ui/host/desktop';
	import shaderIcon from 'golden_ui/style/icons/node/dashboard.svg';
	import refreshIcon from 'golden_ui/style/icons/reset.svg';
	import { SHADER_LIBRARY_SUFFIXES } from '../media/generated/constants';
	import type { ShaderLibraryEntry } from '../media/generated/ShaderLibraryEntry';
	import { beginIsfDrag, browseIsf, endIsfDrag } from './library';

	let props: PanelProps = $props();
	let explorer = $state<{ setPanelState(state: PanelState): void }>();
	export const setPanelState = (state: PanelState) => explorer?.setPanelState(state);
	const desktop = hasDesktopHost();
	let source = $state<'local' | 'isf'>('local');
	let query = $state('');
	let entries = $state.raw<ShaderLibraryEntry[]>([]);
	let requested = false;
	let pending = $state(false);
	let error = $state('');
	const matches = $derived(
		entries.filter(({ label }) => label.toLowerCase().includes(query.trim().toLowerCase()))
	);

	async function load(refresh: boolean) {
		requested = true;
		pending = true;
		error = '';
		try {
			entries = await browseIsf(refresh);
		} catch (cause) {
			error = cause instanceof Error ? cause.message : String(cause);
		} finally {
			pending = false;
		}
	}

	$effect(() => {
		if (desktop && source === 'isf' && !requested) void load(false);
	});
</script>

<section class="shader-library" aria-label="Shader Library">
	<div class="toolbar">
		<select aria-label="Shader library source" bind:value={source}>
			<option value="local">Local</option>
			<option value="isf" disabled={!desktop}>ISF (VIDVOX)</option>
		</select>
		{#if source === 'isf'}
			<input
				type="search"
				aria-label="Search ISF shaders"
				placeholder="Search shaders…"
				bind:value={query} />
			<IconButton
				icon={refreshIcon}
				label="Refresh ISF catalog"
				onclick={() => load(true)}
				disabled={pending} />
		{/if}
	</div>
	<div class="local" hidden={source !== 'local'}>
		<FileExplorerPanel
			{...props}
			title="Local Shaders"
			fileSuffixes={SHADER_LIBRARY_SUFFIXES}
			bind:this={explorer} />
	</div>
	{#if source === 'isf'}
		{#if pending}<div class="status" role="status">Loading ISF catalog…</div>{/if}
		{#if error}<div class="status" role="status">{error}</div>{/if}
		{#if !pending && !error && !matches.length}<div class="status">No shaders found</div>{/if}
		<ul class="entries" aria-label="ISF catalog">
			{#each matches as entry (entry.id)}
				<li
					draggable="true"
					ondragstart={(event) => beginIsfDrag(event, entry.id)}
					ondragend={endIsfDrag}
					title="Drag into Medias to import {entry.label}">
					<img src={shaderIcon} alt="" draggable="false" />
					{entry.label}
				</li>
			{/each}
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
	.entries img {
		inline-size: 0.9rem;
		block-size: 0.9rem;
	}
	.status {
		padding: 0.35rem 0.55rem;
	}
</style>
