<script lang="ts">
	import type { UiNodeDto, UiParamDto } from '../../types';
	import { appState } from '../../store/workbench.svelte';
	import { watcherVisualizerRegistry } from './watcher/watcher-registry';
	import WatcherValueVisualizer from './watcher/WatcherValueVisualizer.svelte';
	import {
		getFixedRange,
		sanitizeWatcherUiSettings,
		WATCHER_DEFAULT_SETTINGS,
		WATCHER_WINDOW_OPTIONS_SEC,
		type WatcherDecimationMode,
		type WatcherRangeMode,
		type WatcherUiSettings,
		type WatcherVectorViewMode
	} from './watcher/watcher-utils';
	import type { WatcherVisualizerEntry } from './watcher/watcher-registry';

	let {
		node,
		persistedSettings = {},
		onSettingsChange
	} = $props<{
		node: UiNodeDto;
		persistedSettings?: Partial<WatcherUiSettings>;
		onSettingsChange?: (next: Partial<WatcherUiSettings>) => void;
	}>();

	const timeWindowOptionsSec = WATCHER_WINDOW_OPTIONS_SEC;
	const settingsCallback = $derived(onSettingsChange ?? (() => {}));

	let session = $derived(appState.session);
	let liveNode = $derived(session?.graph.state.nodesById.get(node.node_id) ?? node);
	let param = $derived.by((): UiParamDto | null => {
		return liveNode.data.kind === 'parameter' ? liveNode.data.param : null;
	});
	let defaultVisualizer = $derived(param ? watcherVisualizerRegistry[param.value.kind] : null);
	let fixedRange = $derived(getFixedRange(param?.constraints));
	let isVectorParameter = $derived(param?.value.kind === 'vec2' || param?.value.kind === 'vec3');

	const valueCurvesVisualizer: WatcherVisualizerEntry = {
		id: 'value',
		label: 'Value Curves',
		component: WatcherValueVisualizer,
		supportsRangeMode: true,
		supportsDecimation: true
	};

	let timeWindowSec = $state(WATCHER_DEFAULT_SETTINGS.timeWindowSec);
	let rangeMode = $state<WatcherRangeMode>(WATCHER_DEFAULT_SETTINGS.rangeMode);
	let decimationMode = $state<WatcherDecimationMode>(WATCHER_DEFAULT_SETTINGS.decimationMode);
	let vectorViewMode = $state<WatcherVectorViewMode>(WATCHER_DEFAULT_SETTINGS.vectorViewMode);

	let timeWindowMs = $derived(timeWindowSec * 1000);
	let streamKey = $derived(
		param ? `${liveNode.node_id}:${param.value.kind}` : `${liveNode.node_id}:none`
	);
	let visualizer = $derived.by((): WatcherVisualizerEntry | null => {
		if (!defaultVisualizer || !param) {
			return null;
		}
		if (isVectorParameter && vectorViewMode === 'curves') {
			return valueCurvesVisualizer;
		}
		return defaultVisualizer;
	});

	const syncPersistedSettings = (): void => {
		const safe = sanitizeWatcherUiSettings(persistedSettings);
		timeWindowSec = safe.timeWindowSec;
		rangeMode = safe.rangeMode;
		decimationMode = safe.decimationMode;
		vectorViewMode = safe.vectorViewMode;
	};

	$effect(() => {
		streamKey;
		syncPersistedSettings();
	});

	const applySettingPatch = (patch: Partial<WatcherUiSettings>): void => {
		settingsCallback(patch);
	};

	const setTimeWindowSec = (nextWindowSec: number): void => {
		if (timeWindowSec === nextWindowSec) {
			return;
		}
		timeWindowSec = nextWindowSec;
		applySettingPatch({ timeWindowSec: nextWindowSec });
	};

	const setRangeMode = (nextRangeMode: WatcherRangeMode): void => {
		if (rangeMode === nextRangeMode) {
			return;
		}
		rangeMode = nextRangeMode;
		applySettingPatch({ rangeMode: nextRangeMode });
	};

	const setDecimationMode = (nextMode: WatcherDecimationMode): void => {
		if (decimationMode === nextMode) {
			return;
		}
		decimationMode = nextMode;
		applySettingPatch({ decimationMode: nextMode });
	};

	const setVectorViewMode = (nextMode: WatcherVectorViewMode): void => {
		if (vectorViewMode === nextMode) {
			return;
		}
		vectorViewMode = nextMode;
		applySettingPatch({ vectorViewMode: nextMode });
	};

	$effect(() => {
		if (!visualizer?.supportsRangeMode && rangeMode !== 'adaptive') {
			rangeMode = 'adaptive';
			applySettingPatch({ rangeMode: 'adaptive' });
		}
		if (rangeMode === 'fixed' && !fixedRange) {
			rangeMode = 'adaptive';
			applySettingPatch({ rangeMode: 'adaptive' });
		}
		if (!visualizer?.supportsDecimation && decimationMode !== 'off') {
			decimationMode = 'off';
			applySettingPatch({ decimationMode: 'off' });
		}
		if (!isVectorParameter && vectorViewMode !== 'phase') {
			vectorViewMode = 'phase';
			applySettingPatch({ vectorViewMode: 'phase' });
		}
	});
</script>

{#if param && visualizer}
	<section class="watcher-panel" data-visualizer={visualizer.id} data-kind={param.value.kind}>
		<header class="watcher-header">
			<div class="watcher-title">
				<strong>Watcher</strong>
				<span>{visualizer.label}</span>
			</div>

			<div class="watcher-controls">
				{#if isVectorParameter}
					<div class="segmented" role="group" aria-label="Vector watcher mode">
						<button
							type="button"
							class:selected={vectorViewMode === 'phase'}
							onclick={() => {
								setVectorViewMode('phase');
							}}>
							Phase
						</button>
						<button
							type="button"
							class:selected={vectorViewMode === 'curves'}
							onclick={() => {
								setVectorViewMode('curves');
							}}>
							Curves
						</button>
					</div>
				{/if}

				<div class="segmented" role="group" aria-label="Watcher time window">
					{#each timeWindowOptionsSec as optionSec}
						<button
							type="button"
							class:selected={timeWindowSec === optionSec}
							onclick={() => {
								setTimeWindowSec(optionSec);
							}}>
							{optionSec}s
						</button>
					{/each}
				</div>

				{#if visualizer.supportsRangeMode}
					<div class="segmented" role="group" aria-label="Watcher range mode">
						<button
							type="button"
							class:selected={rangeMode === 'adaptive'}
							onclick={() => {
								setRangeMode('adaptive');
							}}>
							Adaptive
						</button>
						<button
							type="button"
							class:selected={rangeMode === 'fixed'}
							disabled={!fixedRange}
							title={fixedRange
								? 'Use parameter min/max constraints'
								: 'No min/max constraints available'}
							onclick={() => {
								if (fixedRange) {
									setRangeMode('fixed');
								}
							}}>
							Fixed
						</button>
					</div>
				{/if}

				{#if visualizer.supportsDecimation}
					<div class="segmented" role="group" aria-label="Watcher decimation">
						<button
							type="button"
							class:selected={decimationMode === 'off'}
							onclick={() => {
								setDecimationMode('off');
							}}>
							Raw
						</button>
						<button
							type="button"
							class:selected={decimationMode === 'auto'}
							onclick={() => {
								setDecimationMode('auto');
							}}>
							Auto
						</button>
						<button
							type="button"
							class:selected={decimationMode === 'minmax'}
							onclick={() => {
								setDecimationMode('minmax');
							}}>
							MinMax
						</button>
					</div>
				{/if}
			</div>
		</header>

		<visualizer.component
			sampleValue={param.value}
			constraints={param.constraints}
			{timeWindowMs}
			{rangeMode}
			unit={param.ui_hints.unit ?? ''}
			{decimationMode}
			{streamKey} />
	</section>
{/if}

<style>
	.watcher-panel {
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
		padding: 0.45rem;
		border-radius: 0.5rem;
		border: solid 0.06rem rgba(255 255 255 / 0.11);
		background: linear-gradient(180deg, rgba(255 255 255 / 0.07), rgba(255 255 255 / 0.03));
	}

	.watcher-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: 0.5rem;
	}

	.watcher-title {
		display: inline-flex;
		flex-direction: column;
		line-height: 1.1;
		gap: 0.12rem;
		font-size: 0.68rem;
	}

	.watcher-title strong {
		font-size: 0.74rem;
		letter-spacing: 0.02em;
	}

	.watcher-title span {
		opacity: 0.72;
	}

	.watcher-controls {
		display: inline-flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 0.25rem;
	}

	.segmented {
		display: inline-flex;
		flex-wrap: wrap;
		gap: 0.15rem;
		justify-content: flex-end;
	}

	.segmented button {
		font-size: 0.62rem;
		padding: 0.15rem 0.35rem;
		border-radius: 0.35rem;
		border: solid 0.06rem rgba(255 255 255 / 0.13);
		background: rgba(255 255 255 / 0.05);
		color: inherit;
		opacity: 0.8;
	}

	.segmented button:hover:not(:disabled) {
		opacity: 1;
		background: rgba(255 255 255 / 0.09);
	}

	.segmented button.selected {
		opacity: 1;
		border-color: rgba(130 225 255 / 0.55);
		background: rgba(130 225 255 / 0.18);
	}

	.segmented button:disabled {
		opacity: 0.35;
		cursor: default;
	}
</style>
