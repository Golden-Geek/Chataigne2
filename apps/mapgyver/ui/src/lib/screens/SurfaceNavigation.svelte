<script lang="ts">
	import type { ScreenSnapshot } from './generated/ScreenSnapshot';
	import type { SurfaceId } from './generated/SurfaceId';
	import type { ScreenEdit } from './generated/ScreenEdit';
	let {
		screen,
		selected,
		disabled,
		select,
		edit
	}: {
		screen: ScreenSnapshot;
		selected: SurfaceId | null;
		disabled: boolean;
		select: (surface: SurfaceId) => void;
		edit: (intent: ScreenEdit) => Promise<unknown>;
	} = $props();
	const pageSize = 50;
	let page = $state(0);
	const selectedIndex = $derived(
		screen.surfaces.findIndex((surface) => surface.reference.surface === selected)
	);
	const pages = $derived(Math.max(1, Math.ceil(screen.surfaces.length / pageSize)));
	const currentPage = $derived(Math.min(page, pages - 1));
	const start = $derived(currentPage * pageSize);
	$effect(() => {
		if (selectedIndex >= 0) page = Math.floor(selectedIndex / pageSize);
	});
</script>

<section aria-label="Ordered surfaces">
	<header>
		<h3>Surfaces</h3>
		<button {disabled} onclick={() => edit({ kind: 'AddSurface', screen: screen.id, label: null })}
			>Add surface</button>
	</header>
	<p class="hint">Surfaces are drawn in this order, with the last surface on top.</p>
	{#if screen.surfaces.length === 0}<p>No surfaces. Add one to map a source.</p>{/if}
	{#if pages > 1}<header>
			<button disabled={disabled || currentPage === 0} onclick={() => (page = currentPage - 1)}
				>Previous surfaces</button>
			<small>Page {currentPage + 1} / {pages}</small>
			<button
				disabled={disabled || currentPage === pages - 1}
				onclick={() => (page = currentPage + 1)}>Next surfaces</button>
		</header>{/if}
	<ul>
		{#each screen.surfaces.slice(start, start + pageSize) as surface, offset (surface.reference.surface)}
			{@const index = start + offset}
			<li>
				<button
					class="choice"
					{disabled}
					aria-pressed={selected === surface.reference.surface}
					onclick={() => select(surface.reference.surface)}>{surface.label}</button>
				<button
					aria-label={`Move ${surface.label} up`}
					disabled={disabled || index === 0}
					onclick={() =>
						edit({
							kind: 'ReorderSurface',
							reference: surface.reference,
							after: screen.surfaces[index - 2]?.reference.surface ?? null
						})}>↑</button>
				<button
					aria-label={`Move ${surface.label} down`}
					disabled={disabled || index === screen.surfaces.length - 1}
					onclick={() =>
						edit({
							kind: 'ReorderSurface',
							reference: surface.reference,
							after: screen.surfaces[index + 1].reference.surface
						})}>↓</button>
				<button
					class="danger"
					{disabled}
					onclick={() => edit({ kind: 'RemoveSurface', reference: surface.reference })}
					>Delete</button>
			</li>
		{/each}
	</ul>
	{#if selected}
		{@const surface = screen.surfaces.find((item) => item.reference.surface === selected)}
		{#if surface}
			<form
				class="name"
				onsubmit={(event) => {
					event.preventDefault();
					void edit({
						kind: 'RenameSurface',
						reference: surface.reference,
						label: String(new FormData(event.currentTarget).get('label'))
					});
				}}>
				<input aria-label="Surface name" name="label" value={surface.label} required {disabled} />
				<button {disabled}>Rename surface</button>
			</form>
		{/if}
	{/if}
</section>

<style>
	section {
		display: grid;
		gap: 0.75rem;
	}
	header {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.75rem;
	}
	h3 {
		margin: 0;
		font-size: 1em;
	}
	ul {
		display: grid;
		gap: 0.4rem;
		list-style: none;
		margin: 0;
		padding: 0;
	}
	li {
		display: flex;
		flex-wrap: wrap;
		gap: 0.4rem;
	}
	.choice {
		flex: 1 1 8rem;
		text-align: left;
		overflow-wrap: anywhere;
	}
	.choice[aria-pressed='true'] {
		background: #416caa50;
	}
	.name {
		display: flex;
		gap: 0.4rem;
	}
	.name input {
		flex: 1;
		width: 0;
	}
</style>
