<script lang="ts">
	import Inspector from './Inspector.svelte';
	import NodeTree from './NodeTree.svelte';
	import { setWorkbenchContext } from '../store/workbench-context';
	import type { WorkbenchSession } from '../store/workbench.svelte';
	import type { Snippet } from 'svelte';

	const props = $props<{
		session?: WorkbenchSession | null;
		children?: Snippet;
	}>();

	const session = $derived(props.session ?? null);
	setWorkbenchContext(() => session);
</script>

{#if session}
	<main class="workbench">
		<header class="topbar">
			<div>
				<p class="eyebrow">Golden Core</p>
				<h1>UI Base</h1>
			</div>
			<div class="topbar-right">
				<div class="history-actions">
					<button
						type="button"
						class="history-button"
						title="Ctrl/Cmd+Z"
						disabled={session.historyBusy || !session.canUndo}
						onclick={() => void session.undo()}
					>
						Undo
					</button>
					<button
						type="button"
						class="history-button"
						title="Ctrl/Cmd+Shift+Z or Ctrl+Y"
						disabled={session.historyBusy || !session.canRedo}
						onclick={() => void session.redo()}
					>
						Redo
					</button>
				</div>
				{#if session.status}
					<p class="status">{session.status}</p>
				{/if}
			</div>
		</header>

		<div class="grid">
			<NodeTree />
			<Inspector />
			{@render props.children?.()}
		</div>
	</main>
{:else}
	<main class="workbench">
		<p class="status">Initializing workbench session...</p>
	</main>
{/if}

<style>
	:global(:root) {
		--accent: #ff6e2a;
		--bg-a: #121517;
		--bg-b: #1f2f36;
		--fg: #f0ece4;
		--panel-bg: #1b1f22;
		--panel-border: #2e3a42;
	}

	.workbench {
		min-height: 100dvh;
		padding: 1rem;
		color: var(--fg);
		background:
			radial-gradient(75rem 37.5rem at 10% -10%, color-mix(in srgb, var(--accent) 22%, transparent), transparent),
			linear-gradient(145deg, var(--bg-a), var(--bg-b));
		font-family: 'Space Grotesk', 'Avenir Next', 'Segoe UI', sans-serif;
	}

	.topbar {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.6rem 0.2rem 1rem;
	}

	.topbar-right {
		display: grid;
		justify-items: end;
		gap: 0.4rem;
	}

	.history-actions {
		display: inline-flex;
		gap: 0.4rem;
	}

	.history-button {
		border: none;
		border-radius: 0.5rem;
		background: color-mix(in srgb, var(--panel-bg) 55%, white 45%);
		color: var(--fg);
		font-weight: 700;
		letter-spacing: 0.04em;
		padding: 0.35rem 0.65rem;
		cursor: pointer;
	}

	.history-button:hover:not(:disabled) {
		background: color-mix(in srgb, var(--panel-bg) 45%, white 55%);
	}

	.history-button:disabled {
		opacity: 0.45;
		cursor: not-allowed;
	}

	.eyebrow {
		margin: 0;
		text-transform: uppercase;
		letter-spacing: 0.12em;
		font-size: 0.72rem;
		opacity: 0.75;
	}

	h1 {
		margin: 0.1rem 0 0;
		font-size: clamp(1.5rem, 1.1rem + 2vw, 2.1rem);
	}

	.status {
		margin: 0;
		font-size: 0.82rem;
		opacity: 0.9;
		max-width: min(50ch, 50vw);
		text-align: right;
	}

	.grid {
		display: grid;
		grid-template-columns: minmax(17.5rem, 1.2fr) minmax(17.5rem, 1fr);
		gap: 0.9rem;
	}

	@media (max-width: 53.75rem) {
		.grid {
			grid-template-columns: 1fr;
		}

		.status {
			max-width: none;
			text-align: right;
		}

		.topbar {
			flex-direction: column;
			align-items: flex-start;
		}

		.topbar-right {
			justify-items: start;
		}

		.status {
			text-align: left;
		}
	}
</style>
