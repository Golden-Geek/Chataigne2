<script lang="ts">
	import type { ScreenManagerSnapshot } from './generated/ScreenManagerSnapshot';
	import type { ScreenId } from './generated/ScreenId';
	import type { ScreenEdit } from './generated/ScreenEdit';
	let {
		catalog,
		selected,
		disabled,
		select,
		edit
	}: {
		catalog: ScreenManagerSnapshot;
		selected: ScreenId | null;
		disabled: boolean;
		select: (screen: ScreenId) => void;
		edit: (intent: ScreenEdit) => Promise<unknown>;
	} = $props();
	const pageSize = 50;
	let page = $state(0);
	const selectedIndex = $derived(catalog.screens.findIndex((screen) => screen.id === selected));
	const pages = $derived(Math.max(1, Math.ceil(catalog.screens.length / pageSize)));
	const currentPage = $derived(Math.min(page, pages - 1));
	const start = $derived(currentPage * pageSize);
	$effect(() => {
		if (selectedIndex >= 0) page = Math.floor(selectedIndex / pageSize);
	});
</script>

<nav aria-label="Screens">
	{#if pages > 1}<div class="actions">
			<button disabled={disabled || currentPage === 0} onclick={() => (page = currentPage - 1)}
				>Previous screens</button>
			<small>Page {currentPage + 1} / {pages}</small>
			<button
				disabled={disabled || currentPage === pages - 1}
				onclick={() => (page = currentPage + 1)}>Next screens</button>
		</div>{/if}
	{#each catalog.screens.slice(start, start + pageSize) as screen, offset (screen.id)}
		{@const index = start + offset}
		<article class:active={selected === screen.id}>
			<button
				class="choice"
				{disabled}
				aria-pressed={selected === screen.id}
				onclick={() => select(screen.id)}>
				<strong>{screen.label}</strong><small
					>{screen.surfaces.length} surfaces · {screen.definition.target?.kind ??
						'Unconfigured'}</small>
			</button>
			<form
				class="name"
				onsubmit={(event) => {
					event.preventDefault();
					void edit({
						kind: 'RenameScreen',
						screen: screen.id,
						label: String(new FormData(event.currentTarget).get('label'))
					});
				}}>
				<input
					aria-label={`Screen name: ${screen.label}`}
					name="label"
					value={screen.label}
					required
					{disabled} />
				<button {disabled}>Rename</button>
			</form>
			<div class="actions">
				<button
					aria-label={`Move ${screen.label} up`}
					disabled={disabled || index === 0}
					onclick={() =>
						edit({
							kind: 'ReorderScreen',
							screen: screen.id,
							after: catalog.screens[index - 2]?.id ?? null
						})}>↑</button>
				<button
					aria-label={`Move ${screen.label} down`}
					disabled={disabled || index === catalog.screens.length - 1}
					onclick={() =>
						edit({
							kind: 'ReorderScreen',
							screen: screen.id,
							after: catalog.screens[index + 1].id
						})}>↓</button>
				<button
					class="danger"
					{disabled}
					onclick={() => edit({ kind: 'RemoveScreen', screen: screen.id })}>Delete screen</button>
			</div>
		</article>
	{/each}
</nav>

<style>
	nav {
		display: grid;
		align-content: start;
		gap: 0.8rem;
		min-width: 0;
	}
	article {
		display: grid;
		gap: 0.65rem;
		padding: 0.8rem;
		min-width: 0;
		border: 0.0625rem solid #ffffff24;
		border-radius: 0.5rem;
	}
	article.active {
		border-color: #769ed8;
	}
	.choice {
		display: grid;
		gap: 0.3rem;
		text-align: left;
		overflow-wrap: anywhere;
	}
	small {
		opacity: 0.65;
	}
	.name {
		display: flex;
		gap: 0.4rem;
	}
	.name input {
		flex: 1;
		width: 0;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.4rem;
	}
</style>
