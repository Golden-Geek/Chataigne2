<script lang="ts">
	import parameterTriggerIcon from '../../../../style/icons/parameter/trigger.svg';
	import { TRIGGER_PARAM_EVENT_TOPIC } from '../../../../store/session/custom-events.svelte';
	import { appState } from '../../../../store/workbench.svelte';
	import { sendSetParamIntent } from '../../../../store/ui-intents';
	import type { UiNodeDto } from '../../../../types';

	type TriggerHitSource = 'manual' | 'engine';

	const HIT_DURATION_MS = 140;
	const MANUAL_ECHO_WINDOW_MS = 180;

	const nowMs = (): number => (typeof performance !== 'undefined' ? performance.now() : Date.now());

	let {
		node,
		layoutMode = 'default',
		insideLabel = null,
		readOnly: externalReadOnly = false
	} = $props<{
		node: UiNodeDto;
		layoutMode?: 'default' | 'widget';
		insideLabel?: string | null;
		readOnly?: boolean;
	}>();

	let session = $derived(appState.session);
	let liveNode = $derived(session?.graph.state.nodesById.get(node.node_id) ?? node);
	let param = $derived(liveNode.data.kind === 'parameter' ? liveNode.data.param : null);
	let readOnly = $derived(Boolean(param?.read_only) || externalReadOnly);
	let enabled = $derived(liveNode.meta.enabled);
	let triggerSequence = $derived(
		session?.getCustomEventSequence(TRIGGER_PARAM_EVENT_TOPIC, liveNode.node_id) ?? 0
	);

	let hitTimeout = $state<ReturnType<typeof setTimeout> | null>(null);
	let isHit = $state(false);
	let activeHitSource = $state<TriggerHitSource | null>(null);
	let lastObservedTriggerSequence = $state<number | null>(null);
	let pendingManualEchoDeadlines = $state<number[]>([]);
	let inlineLabel = $derived(typeof insideLabel === 'string' ? insideLabel.trim() : '');
	let showsInlineLabel = $derived(layoutMode === 'widget' && inlineLabel.length > 0);

	const prunePendingManualEchoes = (): void => {
		const currentTime = nowMs();
		pendingManualEchoDeadlines = pendingManualEchoDeadlines.filter(
			(deadline) => deadline > currentTime
		);
	};

	const hit = (source: TriggerHitSource): void => {
		activeHitSource = source;
		isHit = true;
		if (hitTimeout !== null) {
			clearTimeout(hitTimeout);
		}

		hitTimeout = setTimeout(() => {
			isHit = false;
			activeHitSource = null;
		}, HIT_DURATION_MS);
	};

	$effect(() => {
		return () => {
			if (hitTimeout !== null) {
				clearTimeout(hitTimeout);
			}
		};
	});

	$effect(() => {
		const currentSequence = triggerSequence;

		if (lastObservedTriggerSequence === null) {
			lastObservedTriggerSequence = currentSequence;
			return;
		}

		if (currentSequence <= lastObservedTriggerSequence) {
			return;
		}

		const sequenceDelta = currentSequence - lastObservedTriggerSequence;
		lastObservedTriggerSequence = currentSequence;
		prunePendingManualEchoes();

		const suppressedManualEchoes = Math.min(sequenceDelta, pendingManualEchoDeadlines.length);
		if (suppressedManualEchoes > 0) {
			pendingManualEchoDeadlines = pendingManualEchoDeadlines.slice(suppressedManualEchoes);
		}

		if (sequenceDelta > suppressedManualEchoes) {
			hit('engine');
		}
	});

	const fireTrigger = (): void => {
		if (!param || param.value.kind !== 'trigger' || readOnly || !enabled) {
			return;
		}
		pendingManualEchoDeadlines = [...pendingManualEchoDeadlines, nowMs() + MANUAL_ECHO_WINDOW_MS];
		hit('manual');
		void sendSetParamIntent(liveNode.node_id, { kind: 'trigger' }, param.event_behaviour);
	};
</script>

<button
	type="button"
	class="trigger"
	class:widget-layout={layoutMode === 'widget'}
	class:with-inline-label={showsInlineLabel}
	class:active={isHit}
	class:active-manual={isHit && activeHitSource === 'manual'}
	class:active-engine={isHit && activeHitSource === 'engine'}
	disabled={!enabled}
	class:readonly={readOnly}
	onmousedown={fireTrigger}
	onkeydown={(event) => {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			fireTrigger();
		}
	}}>
	<img src={parameterTriggerIcon} alt="Trigger" />
	{#if showsInlineLabel}
		<span class="trigger-label">{inlineLabel}</span>
	{/if}
</button>

<style>
	.trigger {
		color: var(--gc-color-text);
		background: var(--gc-color-trigger);
		border: solid 1px hsl(from var(--gc-color-trigger) h s calc(l * 1.2)) !important;
		border-radius: 0.5rem;
		padding: 0;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 3rem;
		height: 1.2rem;
		box-sizing: border-box;
		filter: brightness(100%);
		transition:
			filter 0.42s ease,
			background-color 0.42s ease,
			border-color 0.42s ease,
			opacity 0.42s ease;
	}

	.trigger:hover {
		filter: brightness(120%);
	}

	.trigger.widget-layout {
		inline-size: 100%;
		block-size: 100%;
		min-block-size: 0;
	}

	.trigger.widget-layout.with-inline-label {
		gap: 0.55rem;
		padding-inline: 0.8rem;
	}

	.trigger:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.trigger.readonly {
		background-color: rgb(from var(--gc-color-readonly) r g b / 20%);
		border-color: rgb(from var(--gc-color-readonly) r g b / 30%) !important;
		cursor: default;
	}

	.trigger.active {
		filter: brightness(122%);
		transition:
			filter 0.06s ease-out,
			background-color 0.06s ease-out,
			border-color 0.06s ease-out,
			opacity 0.06s ease-out;
	}

	.trigger.active-manual {
		background: var(--gc-color-trigger-on);
		border-color: hsl(from var(--gc-color-trigger-on) h s calc(l * 1.2)) !important;
	}

	.trigger.active-engine {
		background: var(--gc-color-trigger-engine);
		border-color: hsl(from var(--gc-color-trigger-engine) h s calc(l * 1.2)) !important;
	}

	.trigger img {
		padding: 0.25rem;
		width: 0.8rem;
		height: 0.8rem;
	}

	.trigger.widget-layout img {
		width: 1.2rem;
		height: 1.2rem;
	}

	.trigger-label {
		font-size: 0.78rem;
		font-weight: 600;
		line-height: 1;
		white-space: nowrap;
	}

	.trigger.trigger.readonly img {
		filter: hue-rotate(160deg) saturate(80%) brightness(80%);
	}
</style>
