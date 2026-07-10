<script lang="ts">
	import {
		clearRuntimeProbe,
		runtimeProbe,
		setRuntimeProbeOverlayEnabled
	} from '../../debug/runtime-probe.svelte';

	let expanded = $state(false);
	const visible = $derived(runtimeProbe.overlayEnabled);
	const entryCountLabel = $derived(
		runtimeProbe.entries.length === 1
			? '1 runtime issue'
			: `${runtimeProbe.entries.length} runtime issues`
	);

	$effect(() => {
		if (!visible) {
			expanded = false;
			return;
		}
		if (runtimeProbe.overlayEnabled) {
			expanded = true;
		}
	});
</script>

{#if visible}
	<aside class:expanded class="runtime-probe-overlay" aria-live="polite">
		<div class="runtime-probe-header">
			<button
				type="button"
				class="runtime-probe-toggle"
				onclick={() => {
					expanded = !expanded;
				}}>{entryCountLabel}</button>
			<div class="runtime-probe-actions">
				<button
					type="button"
					onclick={() => {
						clearRuntimeProbe();
					}}>Clear</button>
				<button
					type="button"
					onclick={() => {
						setRuntimeProbeOverlayEnabled(false);
						expanded = runtimeProbe.entries.length > 0;
					}}>Hide</button>
			</div>
		</div>

		{#if expanded}
			<div class="runtime-probe-body">
				{#if runtimeProbe.entries.length === 0}
					<p>No runtime issues recorded.</p>
				{:else}
					<ol class="runtime-probe-list">
						{#each runtimeProbe.entries as entry (entry.id)}
							<li>
								<header>
									<strong>{entry.source}</strong>
									<span>{entry.timestamp}</span>
								</header>
								<p>{entry.message}</p>
								{#if entry.detail}
									<div class="runtime-probe-detail">{entry.detail}</div>
								{/if}
								{#if entry.stack}
									<pre>{entry.stack}</pre>
								{/if}
							</li>
						{/each}
					</ol>
				{/if}
			</div>
		{/if}
	</aside>
{/if}

<style>
	.runtime-probe-overlay {
		position: fixed;
		inset-inline-end: 1rem;
		inset-block-end: 1rem;
		inline-size: min(32rem, calc(100vw - 2rem));
		max-block-size: min(28rem, calc(100vh - 2rem));
		display: flex;
		flex-direction: column;
		gap: 0.65rem;
		padding: 0.8rem;
		border-radius: 0.9rem;
		background: rgb(from var(--gc-color-background) r g b / 0.94);
		border: solid 0.06rem rgb(from var(--gc-color-focus) r g b / 0.55);
		box-shadow:
			0 0.8rem 2.2rem rgb(from var(--gc-color-background) r g b / 0.3),
			0 0 0 0.08rem rgb(from var(--gc-color-focus) r g b / 0.14);
		z-index: 1100;
	}

	.runtime-probe-overlay:not(.expanded) {
		inline-size: auto;
		max-inline-size: calc(100vw - 2rem);
	}

	.runtime-probe-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.6rem;
	}

	.runtime-probe-toggle,
	.runtime-probe-actions button {
		border-radius: 999rem;
		border: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.55);
		background: rgb(from var(--gc-color-background) r g b / 0.56);
		color: var(--gc-color-text);
		padding: 0.35rem 0.8rem;
		font-size: 0.72rem;
	}

	.runtime-probe-toggle {
		font-weight: 700;
		letter-spacing: 0.03em;
	}

	.runtime-probe-actions {
		display: flex;
		align-items: center;
		gap: 0.4rem;
	}

	.runtime-probe-body {
		overflow: auto;
		padding-inline-end: 0.2rem;
	}

	.runtime-probe-body p {
		margin: 0;
		font-size: 0.76rem;
		line-height: 1.45;
	}

	.runtime-probe-list {
		display: flex;
		flex-direction: column;
		gap: 0.7rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.runtime-probe-list li {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
		padding: 0.65rem 0.75rem;
		border-radius: 0.75rem;
		background: rgb(from var(--gc-color-background) r g b / 0.44);
		border: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.34);
	}

	.runtime-probe-list header {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 0.6rem;
		font-size: 0.68rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		opacity: 0.78;
	}

	.runtime-probe-detail {
		font-size: 0.72rem;
		opacity: 0.76;
	}

	.runtime-probe-list pre {
		margin: 0;
		padding: 0.55rem 0.65rem;
		border-radius: 0.6rem;
		background: rgb(from var(--gc-color-background) r g b / 0.5);
		font-size: 0.68rem;
		line-height: 1.45;
		white-space: pre-wrap;
		word-break: break-word;
		overflow-wrap: anywhere;
	}
</style>
