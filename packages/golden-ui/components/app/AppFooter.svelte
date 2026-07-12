<script lang="ts">
	import type { WorkbenchSession } from '../../store/workbench.svelte';

	const { session } = $props<{
		session?: WorkbenchSession | null;
	}>();

	let hoverInfo = $derived(session?.getFooterHoverInfo() ?? null);
</script>

<footer class="gc-footer">
	{#if session}
		<div class="footer-description" aria-hidden={hoverInfo === null}>
			{#if hoverInfo}
				<span class="description-label">{hoverInfo.label}</span>
				<span class="description-separator">:</span>
				<span class="description-copy">{hoverInfo.description}</span>
			{/if}
		</div>
		<div class="footer-toaster" aria-live="polite">
			{#each session.toasts as toast (toast.id)}
				<div class="toast {toast.level}" role="status">
					<span class="toast-message">{toast.message}</span>
					<button
						type="button"
						class="toast-dismiss"
						aria-label="Dismiss notification"
						onclick={() => session.dismissToast(toast.id)}>
						x
					</button>
				</div>
			{/each}
		</div>
	{/if}
</footer>

<style>
	.gc-footer {
		flex: 0 0 2rem;
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.75rem;
		min-height: 2rem;
		padding: 0 1rem;
		overflow: hidden;
	}

	.footer-description {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		min-width: 0;
		overflow: hidden;
		font-size: 0.72rem;
		line-height: 1.15;
		color: rgb(from var(--gc-color-text) r g b / 82%);
	}

	.description-label,
	.description-copy {
		min-width: 0;
	}

	.description-label {
		flex: 0 1 auto;
		font-weight: 600;
		color: var(--gc-color-text);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.description-separator {
		flex: 0 0 auto;
		color: rgb(from var(--gc-color-text) r g b / 48%);
	}

	.description-copy {
		flex: 1 1 auto;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.footer-toaster {
		display: flex;
		flex-direction: row-reverse;
		align-items: center;
		gap: 0.5rem;
		justify-self: end;
		min-width: 0;
		max-width: 100%;
		overflow: hidden;
	}

	.toast {
		flex: 0 1 14rem;
		min-width: 0;
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		max-width: 14rem;
		padding: 0.32rem 0.55rem 0.32rem 0.65rem;
		border-radius: 0.65rem;
		border: 0.05rem solid rgb(from var(--gc-color-text) r g b / 16%);
		background: linear-gradient(
			135deg,
			rgb(0 0 0 / 42%),
			rgb(from var(--gc-color-footer) r g b / 96%)
		);
		box-shadow: 0 0.25rem 0.8rem rgb(0 0 0 / 28%);
		backdrop-filter: blur(0.6rem);
	}

	.toast.info {
		border-color: rgb(from var(--gc-color-focus) r g b / 28%);
		background-color: rgb(from var(--gc-color-focus) r g b / 12%);
	}

	.toast.success {
		border-color: rgb(from var(--gc-color-success) r g b / 28%);
		background-color: rgb(from var(--gc-color-success) r g b / 12%);
	}

	.toast.warning {
		border-color: rgb(from var(--gc-color-warning) r g b / 28%);
		background-color: rgb(from var(--gc-color-warning) r g b / 12%);
	}

	.toast.error {
		border-color: rgb(from var(--gc-color-error) r g b / 28%);
		background-color: rgb(from var(--gc-color-error) r g b / 12%);
	}

	.toast-message {
		flex: 1 1 auto;
		min-width: 0;
		font-size: 0.68rem;
		line-height: 1.1;
		color: var(--gc-color-text);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.toast-dismiss {
		flex: 0 0 auto;
		width: 1rem;
		height: 1rem;
		border-radius: 999rem;
		font-size: 0.62rem;
		line-height: 1;
		color: rgb(from var(--gc-color-text) r g b / 72%);
		background-color: rgb(from var(--gc-color-text) r g b / 8%);
	}

	@media (max-width: 48rem) {
		.gc-footer {
			gap: 0.5rem;
			padding: 0 0.75rem;
		}

		.footer-description {
			font-size: 0.68rem;
		}

		.footer-toaster {
			gap: 0.35rem;
		}

		.toast {
			flex-basis: 9rem;
			max-width: 9rem;
		}
	}
</style>
