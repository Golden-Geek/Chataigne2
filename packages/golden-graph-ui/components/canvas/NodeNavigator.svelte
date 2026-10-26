<script lang="ts">
	import { tick } from 'svelte';
	import type { CanvasNavigation } from './navigation.svelte';
	let {
		navigation,
		panelId,
		returnFocus,
		findLabel
	}: {
		navigation: CanvasNavigation;
		panelId: string;
		returnFocus: () => void;
		findLabel?: string;
	} = $props();
	let input = $state<HTMLInputElement>();
	$effect(() => {
		if (navigation.open) {
			let retired = false;
			void tick().then(() => {
				if (!retired) input?.focus();
			});
			return () => {
				retired = true;
			};
		}
	});
	const close = () => {
		navigation.close();
		returnFocus();
	};
	$effect.pre(() => {
		navigation.rows;
		if (!navigation.open) return;
		const focused = document.activeElement;
		if (!(focused instanceof HTMLElement) || !focused.hasAttribute('data-graph-result')) return;
		let retired = false;
		void tick().then(() => {
			if (
				!retired &&
				navigation.open &&
				!focused.isConnected &&
				document.activeElement === document.body
			)
				input?.focus();
		});
		return () => {
			retired = true;
		};
	});
</script>

{#if navigation.open}
	<!-- Escape is delegated from the search and paging controls. -->
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<section
		id={panelId}
		class="node-navigator"
		aria-label={findLabel ?? 'Find graph nodes'}
		onpointerdown={(event) => event.stopPropagation()}
		onwheel={(event) => event.stopPropagation()}
		oncontextmenu={(event) => event.stopPropagation()}
		ondblclick={(event) => event.stopPropagation()}
		onkeydown={(event) => {
			event.stopPropagation();
			if (event.key === 'Escape') {
				event.preventDefault();
				close();
			}
		}}>
		<label
			>{findLabel ?? 'Find node'}
			<input
				bind:this={input}
				type="search"
				value={navigation.query}
				oninput={(event) => navigation.search(event.currentTarget.value)} />
		</label>
		<p role="status" aria-live="polite">
			{navigation.busy
				? 'Searching…'
				: `${navigation.rows.length} results on page ${navigation.pageNumber}${navigation.hasNext ? '; more available' : ''}.`}
		</p>
		<ul aria-label="Matching nodes" aria-busy={navigation.busy}>
			{#each navigation.rows as node (node.id)}
				<li>
					<button
						type="button"
						data-graph-result={node.id}
						aria-disabled={navigation.busy}
						onclick={(event) =>
							!navigation.busy &&
							navigation.reveal(node.id, event.ctrlKey || event.metaKey || event.shiftKey)}
						title={node.id}>{node.label} <small>{node.id}</small></button>
				</li>
			{/each}
		</ul>
		<div class="paging">
			<button
				type="button"
				disabled={navigation.busy || !navigation.hasPrevious}
				onclick={() => navigation.page('previous')}>Previous</button>
			<button
				type="button"
				disabled={navigation.busy || !navigation.hasNext}
				onclick={() => navigation.page('next')}>Next</button>
			<button type="button" onclick={close}>Close</button>
		</div>
	</section>
{/if}

<style>
	.node-navigator {
		position: absolute;
		z-index: 21;
		inset-block-start: 3.5rem;
		inset-inline-end: 0.7rem;
		inline-size: min(26rem, calc(100% - 1.4rem));
		max-block-size: min(32rem, calc(100% - 4.2rem));
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 0.75rem;
		background: var(--ga-node);
		border: 0.1rem solid var(--ga-outline);
		border-radius: 0.4rem;
		box-sizing: border-box;
		color: var(--gc-color-text, #d8dfeb);
	}
	label {
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}
	input {
		min-inline-size: 0;
		flex: 1;
	}
	button,
	input {
		border: 0.06rem solid var(--ga-outline);
		border-radius: 0.25rem;
		padding: 0.3rem 0.5rem;
		background: var(--ga-bg);
		color: inherit;
		font: inherit;
	}
	button {
		cursor: pointer;
	}
	button:disabled {
		opacity: 0.5;
		cursor: default;
	}
	p {
		margin: 0;
		font-size: 0.8rem;
	}
	ul {
		list-style: none;
		padding: 0;
		margin: 0;
		overflow: auto;
		min-block-size: 0;
	}
	li button {
		inline-size: 100%;
		text-align: start;
		overflow-wrap: anywhere;
	}
	small {
		opacity: 0.7;
	}
	.paging {
		display: flex;
		gap: 0.5rem;
	}
</style>
