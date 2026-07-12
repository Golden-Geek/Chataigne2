<script lang="ts">
	import { cubicOut } from 'svelte/easing';
	import { fade, scale } from 'svelte/transition';

	import type { WorkbenchLoadingStepStatus, WorkbenchSession } from '../../store/workbench.svelte';

	const { session } = $props<{
		session: WorkbenchSession;
	}>();

	const reduceMotion =
		typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
	const overlayTransition = { duration: reduceMotion ? 0 : 220 };
	const panelTransition = {
		start: reduceMotion ? 1 : 0.985,
		duration: reduceMotion ? 0 : 260,
		easing: cubicOut
	};

	let loading = $derived(session.loading);
	let progressPercent = $derived(Math.round(loading.progress * 100));
	let progressTransform = $derived(`scaleX(${Math.max(0.015, loading.progress)})`);

	const stepStatusLabel = (status: WorkbenchLoadingStepStatus): string => {
		if (status === 'complete') {
			return 'Done';
		}
		if (status === 'active') {
			return 'Now';
		}
		if (status === 'attention') {
			return 'Retrying';
		}
		return 'Waiting';
	};
</script>

{#if loading.visible}
	<section
		class="gc-loading-overlay tone-{loading.tone}"
		role="status"
		aria-live="polite"
		aria-busy="true"
		aria-label={loading.title}
		transition:fade={overlayTransition}>
		<div class="gc-loading-panel" transition:scale={panelTransition}>
			<div class="gc-loading-heading">
				<div class="gc-loading-title-row">
					<span class="gc-loading-activity" aria-hidden="true"></span>
					<h1>{loading.title}</h1>
				</div>
				<p>{loading.message}</p>
				{#if loading.detail}
					<p class="gc-loading-detail">{loading.detail}</p>
				{/if}
			</div>

			<div
				class="gc-loading-progress"
				role="progressbar"
				aria-valuemin="0"
				aria-valuemax="100"
				aria-valuenow={progressPercent}
				aria-label={`${loading.title}: ${progressPercent}%`}>
				<div class="gc-loading-progress-meta">
					<span>Progress</span>
					<span>{progressPercent}%</span>
				</div>
				<div class="gc-loading-progress-track" aria-hidden="true">
					<span class="gc-loading-progress-fill" style={`transform: ${progressTransform}`}></span>
				</div>
			</div>

			<ol class="gc-loading-steps" aria-label="Loading steps">
				{#each loading.steps as step}
					<li class="gc-loading-step {step.status}">
						<span class="gc-loading-step-marker" aria-hidden="true"></span>
						<span class="gc-loading-step-label">{step.label}</span>
						<span class="gc-loading-step-status">{stepStatusLabel(step.status)}</span>
					</li>
				{/each}
			</ol>
		</div>
	</section>
{/if}

<style>
	.gc-loading-overlay {
		position: fixed;
		inset: 0;
		z-index: 1000;
		display: grid;
		place-items: center;
		box-sizing: border-box;
		padding: 1.25rem;
		color: var(--gc-color-text);
		background:
			linear-gradient(135deg, rgb(0 0 0 / 18%), rgb(from var(--gc-color-background) r g b / 46%)),
			rgb(from var(--gc-color-background) r g b / 18%);
		backdrop-filter: blur(0.42rem) saturate(1.08);
		-webkit-backdrop-filter: blur(0.42rem) saturate(1.08);
	}

	.gc-loading-panel {
		--gc-loading-accent: var(--gc-color-focus);
		--gc-loading-secondary: var(--gc-color-success);

		position: relative;
		inline-size: min(34rem, 100%);
		box-sizing: border-box;
		padding: 1.35rem;
		border: 0.0625rem solid rgb(from var(--gc-loading-accent) r g b / 28%);
		border-radius: 0.5rem;
		background:
			linear-gradient(180deg, rgb(255 255 255 / 10%), rgb(from var(--gc-color-panel) r g b / 44%)),
			rgb(from var(--gc-color-panel) r g b / 72%);
		box-shadow:
			0 1rem 2.6rem rgb(0 0 0 / 28%),
			inset 0 0.0625rem 0 rgb(255 255 255 / 16%);
		backdrop-filter: blur(0.7rem) saturate(1.16);
		-webkit-backdrop-filter: blur(0.7rem) saturate(1.16);
		overflow: hidden;
	}

	.gc-loading-overlay.tone-attention .gc-loading-panel {
		--gc-loading-accent: var(--gc-color-warning);
		--gc-loading-secondary: var(--gc-color-focus);
	}

	.gc-loading-overlay.tone-ready .gc-loading-panel {
		--gc-loading-accent: var(--gc-color-success);
		--gc-loading-secondary: var(--gc-color-focus);
	}

	.gc-loading-panel::before {
		content: '';
		position: absolute;
		inset-block-start: 0;
		inset-inline: 0;
		block-size: 0.16rem;
		background: linear-gradient(
			90deg,
			var(--gc-loading-accent),
			var(--gc-loading-secondary),
			var(--gc-loading-accent)
		);
		opacity: 0.9;
	}

	.gc-loading-heading {
		display: grid;
		gap: 0.45rem;
		margin-block-end: 1.15rem;
	}

	.gc-loading-title-row {
		display: flex;
		align-items: center;
		gap: 0.7rem;
		min-inline-size: 0;
	}

	.gc-loading-activity {
		flex: 0 0 auto;
		inline-size: 0.85rem;
		block-size: 0.85rem;
		border-radius: 50%;
		border: 0.12rem solid rgb(from var(--gc-loading-accent) r g b / 26%);
		border-block-start-color: var(--gc-loading-accent);
		box-shadow: 0 0 0.8rem rgb(from var(--gc-loading-accent) r g b / 26%);
		animation: gc-loading-spin 900ms linear infinite;
	}

	.gc-loading-heading h1 {
		margin: 0;
		min-inline-size: 0;
		overflow-wrap: anywhere;
		font-size: 1.05rem;
		line-height: 1.15;
		font-weight: 800;
		color: rgb(from var(--gc-color-text) r g b / 94%);
	}

	.gc-loading-heading p {
		margin: 0;
		font-size: 0.82rem;
		line-height: 1.45;
		color: rgb(from var(--gc-color-text) r g b / 72%);
	}

	.gc-loading-heading .gc-loading-detail {
		color: rgb(from var(--gc-loading-accent) r g b / 82%);
	}

	.gc-loading-progress {
		display: grid;
		gap: 0.45rem;
	}

	.gc-loading-progress-meta {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
		font-size: 0.72rem;
		line-height: 1;
		font-weight: 700;
		letter-spacing: 0.04em;
		text-transform: uppercase;
		color: rgb(from var(--gc-color-text) r g b / 64%);
	}

	.gc-loading-progress-track {
		position: relative;
		block-size: 0.45rem;
		border-radius: 999rem;
		background: rgb(from var(--gc-color-text) r g b / 10%);
		overflow: hidden;
		box-shadow: inset 0 0 0 0.0625rem rgb(from var(--gc-color-text) r g b / 7%);
	}

	.gc-loading-progress-fill {
		position: absolute;
		inset: 0;
		transform-origin: left center;
		border-radius: inherit;
		background: linear-gradient(90deg, var(--gc-loading-accent), var(--gc-loading-secondary));
		transition: transform 420ms cubic-bezier(0.2, 0.8, 0.2, 1);
		overflow: hidden;
	}

	.gc-loading-progress-fill::after {
		content: '';
		position: absolute;
		inset-block: 0;
		inline-size: 35%;
		inset-inline-start: -40%;
		background: linear-gradient(90deg, transparent, rgb(255 255 255 / 55%), transparent);
		animation: gc-loading-sheen 1300ms ease-in-out infinite;
	}

	.gc-loading-steps {
		display: grid;
		gap: 0.55rem;
		margin: 1.2rem 0 0;
		padding: 0;
		list-style: none;
	}

	.gc-loading-step {
		display: grid;
		grid-template-columns: 1rem minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.65rem;
		min-inline-size: 0;
		color: rgb(from var(--gc-color-text) r g b / 52%);
	}

	.gc-loading-step-marker {
		inline-size: 0.68rem;
		block-size: 0.68rem;
		justify-self: center;
		border-radius: 50%;
		border: 0.0625rem solid currentColor;
		background: transparent;
		box-sizing: border-box;
		transition:
			background-color 180ms ease,
			border-color 180ms ease,
			box-shadow 180ms ease,
			transform 180ms ease;
	}

	.gc-loading-step-label {
		min-inline-size: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 0.78rem;
		line-height: 1.2;
		font-weight: 650;
	}

	.gc-loading-step-status {
		font-size: 0.68rem;
		line-height: 1;
		color: rgb(from currentColor r g b / 72%);
	}

	.gc-loading-step.complete {
		color: rgb(from var(--gc-color-success) r g b / 72%);
	}

	.gc-loading-step.complete .gc-loading-step-marker {
		background: currentColor;
		border-color: currentColor;
		box-shadow: 0 0 0.45rem rgb(from currentColor r g b / 28%);
	}

	.gc-loading-step.active,
	.gc-loading-step.attention {
		color: var(--gc-loading-accent);
	}

	.gc-loading-step.active .gc-loading-step-marker,
	.gc-loading-step.attention .gc-loading-step-marker {
		border-color: currentColor;
		background: rgb(from currentColor r g b / 18%);
		box-shadow: 0 0 0.65rem rgb(from currentColor r g b / 38%);
		animation: gc-loading-pulse 1200ms ease-in-out infinite;
	}

	.gc-loading-step.pending {
		color: rgb(from var(--gc-color-text) r g b / 38%);
	}

	@keyframes gc-loading-spin {
		to {
			transform: rotate(360deg);
		}
	}

	@keyframes gc-loading-sheen {
		to {
			transform: translateX(400%);
		}
	}

	@keyframes gc-loading-pulse {
		50% {
			transform: scale(1.18);
			box-shadow: 0 0 0.95rem rgb(from currentColor r g b / 48%);
		}
	}

	@media (max-width: 40rem) {
		.gc-loading-overlay {
			padding: 0.8rem;
		}

		.gc-loading-panel {
			padding: 1rem;
		}

		.gc-loading-step {
			grid-template-columns: 0.9rem minmax(0, 1fr);
			gap: 0.5rem;
		}

		.gc-loading-step-status {
			display: none;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.gc-loading-activity,
		.gc-loading-progress-fill::after,
		.gc-loading-step.active .gc-loading-step-marker,
		.gc-loading-step.attention .gc-loading-step-marker {
			animation: none;
		}

		.gc-loading-progress-fill,
		.gc-loading-step-marker {
			transition: none;
		}
	}
</style>
