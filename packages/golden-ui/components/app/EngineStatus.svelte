<script lang="ts">
	import type { WorkbenchSession } from '../../store/workbench.svelte';

	const { session } = $props<{ session: WorkbenchSession }>();
	const ACTIVITY_DELAY_MS = 300;
	let hasPendingActions = $derived(session.userActionPendingCount > 0);
	let activityVisible = $state(false);
	let activity = $state({ label: '', phase: '', phaseLabel: '', count: 0 });
	let activityTitle = $derived(`${activity.label} — ${activity.phaseLabel}`);
	let phaseLabel = $derived.by((): string => {
		switch (session.userActionCurrentPhase) {
			case 'queued':
				return 'Queued';
			case 'received':
				return 'Received';
			case 'applied':
				return 'Finishing';
			case 'rejected':
				return 'Failed';
			default:
				return 'Processing';
		}
	});

	// Phase/count updates must not restart the delay while work remains pending.
	$effect(() => {
		if (!hasPendingActions) {
			activityVisible = false;
			return;
		}
		const timer = setTimeout(() => {
			activityVisible = true;
		}, ACTIVITY_DELAY_MS);
		return () => clearTimeout(timer);
	});

	// Keep the last pending description during fade-out instead of flashing a fallback.
	$effect(() => {
		if (!hasPendingActions) return;
		const phase = session.userActionCurrentPhase ?? 'accepted';
		activity = {
			label: session.userActionCurrentLabel ?? 'Applying change',
			phase,
			phaseLabel,
			count: session.userActionPendingCount
		};
	});

	let engineRateClass = $derived.by((): string => {
		const hz = session.engineHz;
		if (hz === null) return 'unknown';
		const lowFrequency = Math.max(1, session.engineLowFrequencyHz);
		if (hz < lowFrequency / 2) return 'low';
		if (hz <= lowFrequency) return 'mid';
		return 'high';
	});
	let engineRateLabel = $derived(
		session.engineHz === null ? '-- Hz' : `${Math.round(session.engineHz)} Hz`
	);
	let engineRateDetail = $derived.by((): string => {
		const stats = session.runtimeStats;
		if (!stats) return 'Runtime metrics unavailable';
		const averageQueueMs =
			stats.control_applied > 0 ? stats.control_wait_ns / stats.control_applied / 1_000_000 : 0;
		return [
			`Generation ${stats.generation_id}`,
			`control queue ${stats.control_queue_depth} (peak ${stats.control_queue_peak}, avg wait ${averageQueueMs.toFixed(2)} ms)`,
			`batches ${stats.sparse_batches} sparse / ${stats.dense_batches} dense`,
			`effects ${stats.effects_committed} committed / ${stats.effects_suppressed} suppressed`
		].join(' · ');
	});
</script>

<div class="engine-status">
	<div class="activity-slot">
		<div
			class="user-action-activity"
			class:visible={activityVisible}
			role="status"
			aria-live="polite"
			aria-hidden={!activityVisible}
			aria-label={activityTitle}
			title={activityTitle}
			data-phase={activity.phase}>
			<span class="user-action-spinner" aria-hidden="true"></span>
			<span class="user-action-label">{activity.label}</span>
			<span class="user-action-phase">{activity.phaseLabel}</span>
			{#if activity.count > 1}
				<span class="user-action-count" aria-label={`${activity.count} pending actions`}>
					{activity.count}
				</span>
			{/if}
		</div>
	</div>
	<span class="engine-rate {engineRateClass}" title={engineRateDetail}>
		<span class="engine-rate-dot" aria-hidden="true"></span>
		<span>{engineRateLabel}</span>
	</span>
</div>

<style>
	.engine-status {
		display: flex;
		align-items: center;
		justify-self: end;
		gap: 0.5rem;
	}

	.activity-slot {
		display: flex;
		justify-content: flex-end;
		width: min(28vw, 18rem);
		min-width: 0;
	}

	.user-action-activity {
		display: inline-flex;
		align-items: center;
		gap: 0.38rem;
		min-width: 0;
		max-width: 100%;
		padding: 0.24rem 0.52rem;
		border: 0.05rem solid rgb(from var(--gc-color-accent) r g b / 32%);
		border-radius: 999rem;
		color: rgb(from var(--gc-color-text) r g b / 88%);
		background-color: rgb(from var(--gc-color-accent) r g b / 13%);
		font-size: 0.66rem;
		font-weight: 650;
		line-height: 1;
		pointer-events: none;
		opacity: 0;
		visibility: hidden;
		transition:
			opacity 180ms ease,
			visibility 0s 180ms;
	}

	.user-action-activity.visible {
		opacity: 1;
		visibility: visible;
		transition-delay: 0s;
	}

	.user-action-spinner {
		flex: 0 0 auto;
		width: 0.58rem;
		height: 0.58rem;
		border: 0.1rem solid rgb(from var(--gc-color-accent) r g b / 30%);
		border-top-color: var(--gc-color-accent);
		border-radius: 50%;
		animation: user-action-spin 0.8s linear infinite paused;
	}

	.visible .user-action-spinner {
		animation-play-state: running;
	}

	.user-action-label {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.user-action-phase {
		flex: 0 0 auto;
		color: rgb(from var(--gc-color-text) r g b / 55%);
		font-weight: 550;
	}

	.user-action-count {
		display: inline-grid;
		flex: 0 0 auto;
		min-width: 1.05rem;
		height: 1.05rem;
		padding-inline: 0.24rem;
		place-items: center;
		border-radius: 999rem;
		color: var(--gc-color-background);
		background-color: var(--gc-color-accent);
		font-size: 0.58rem;
		font-variant-numeric: tabular-nums;
	}

	.user-action-activity[data-phase='rejected'] {
		border-color: rgb(from var(--gc-color-error) r g b / 38%);
		color: var(--gc-color-error);
		background-color: rgb(from var(--gc-color-error) r g b / 13%);
	}

	.user-action-activity[data-phase='rejected'] .user-action-spinner {
		animation: none;
		border-color: var(--gc-color-error);
	}

	.engine-rate {
		display: inline-flex;
		flex: 0 0 auto;
		align-items: center;
		gap: 0.35rem;
		min-width: 3.8rem;
		padding: 0.15rem 0.42rem;
		border: 0.05rem solid currentColor;
		border-radius: 0.35rem;
		font-size: 0.68rem;
		font-weight: 700;
		font-variant-numeric: tabular-nums;
		line-height: 1;
		white-space: nowrap;
		color: rgb(from var(--gc-color-text) r g b / 45%);
		background-color: rgb(from var(--gc-color-text) r g b / 7%);
	}

	.engine-rate.low {
		color: var(--gc-color-error);
		background-color: rgb(from var(--gc-color-error) r g b / 12%);
	}

	.engine-rate.mid {
		color: var(--gc-color-warning);
		background-color: rgb(from var(--gc-color-warning) r g b / 12%);
	}

	.engine-rate.high {
		color: var(--gc-color-success);
		background-color: rgb(from var(--gc-color-success) r g b / 12%);
	}

	.engine-rate-dot {
		width: 0.45rem;
		height: 0.45rem;
		border-radius: 50%;
		background-color: currentColor;
		box-shadow: 0 0 0.35rem currentColor;
	}

	@keyframes user-action-spin {
		to {
			transform: rotate(1turn);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.user-action-activity {
			transition: none;
		}

		.user-action-spinner {
			animation: none;
			border-color: var(--gc-color-accent);
			opacity: 0.75;
		}
	}
</style>
