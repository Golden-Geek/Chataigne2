<script lang="ts">
	import {
		AudioDeviceSelector,
		type AudioDeviceInspectorState,
		type AudioDirection
	} from 'golden_audio_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import type { NodeId, PanelProps, PanelState, UiNodeDto } from 'golden_ui';
	import {
		SOUND_CARD_TELEMETRY_TOPIC,
		createSoundCardAudioDeviceInspectorAdapter
	} from '$lib/modules/audio/sound-card/audio-device-inspector-adapter.svelte';
	import type { SoundCardUiTelemetryDto } from '$lib/modules/audio/sound-card/generated';
	import { resolveModuleEditor } from './module-editor-registry';
	import SoundCardAnalysisView from './sound-card/SoundCardAnalysisView.svelte';
	import SoundCardDiagnostics from './sound-card/SoundCardDiagnostics.svelte';
	import SoundCardMeters from './sound-card/SoundCardMeters.svelte';
	import SoundCardNodeSection from './sound-card/SoundCardNodeSection.svelte';
	import SoundCardPlaybackStatus from './sound-card/SoundCardPlaybackStatus.svelte';
	import SoundCardRouteMatrix from './sound-card/SoundCardRouteMatrix.svelte';
	import {
		numericParameterAtPath,
		soundCardChannelLabels,
		soundCardDirectChildrenByType,
		soundCardNodeAtPath,
		soundCardPhysicalChannelEndpoints,
		soundCardPlaybackSourceEndpoints,
		soundCardProfileKey,
		soundCardRouteRecords,
		soundCardVirtualChannelEndpoints,
		stringParameterAtPath,
		type SoundCardMatrixEndpoint
	} from './sound-card/sound-card-editor-model';

	type EditorParams = {
		moduleNodeId?: NodeId;
		inputProfileNodeId?: NodeId;
		outputProfileNodeId?: NodeId;
	};

	const SOUND_CARD_MODULE_TYPE = 'sound_card_module';
	let inputPhysicalEndpointCache: {
		key: string;
		value: readonly SoundCardMatrixEndpoint[];
	} = { key: '', value: [] };
	let outputPhysicalEndpointCache: {
		key: string;
		value: readonly SoundCardMatrixEndpoint[];
	} = { key: '', value: [] };

	const stablePhysicalEndpoints = (
		state: AudioDeviceInspectorState,
		direction: AudioDirection
	): readonly SoundCardMatrixEndpoint[] => {
		const next = soundCardPhysicalChannelEndpoints(state, direction);
		const key = next.map((endpoint) => `${endpoint.key}:${endpoint.label}`).join('\u001f');
		const cache = direction === 'input' ? inputPhysicalEndpointCache : outputPhysicalEndpointCache;
		if (cache.key === key) return cache.value;
		const updated = { key, value: next };
		if (direction === 'input') inputPhysicalEndpointCache = updated;
		else outputPhysicalEndpointCache = updated;
		return next;
	};

	let props: PanelProps = $props();
	let updatedPanelState = $state<PanelState | null>(null);
	let panelState = $derived(
		updatedPanelState ?? {
			panelId: props.panelId,
			panelType: props.panelType,
			title: props.title,
			params: props.params
		}
	);

	export const setPanelState = (next: PanelState): void => {
		updatedPanelState = next;
	};

	let session = $derived(appState.session);
	let nodes = $derived(session?.graph.state.nodesById ?? new Map<NodeId, UiNodeDto>());
	let panelParams = $derived((panelState.params ?? {}) as EditorParams);
	let soundCardModules = $derived(
		[...nodes.values()].filter((node) => node.node_type === SOUND_CARD_MODULE_TYPE)
	);
	let activeModule = $derived.by(() => {
		const requested = panelParams.moduleNodeId;
		if (requested !== undefined) {
			const candidate = nodes.get(requested);
			if (candidate?.node_type === SOUND_CARD_MODULE_TYPE) return candidate;
		}
		return soundCardModules[0] ?? null;
	});
	let binding = $derived(
		activeModule ? createSoundCardAudioDeviceInspectorAdapter(activeModule.node_id) : null
	);
	let telemetry = $derived(
		activeModule
			? (session?.getCustomEventPayload<SoundCardUiTelemetryDto>(
					SOUND_CARD_TELEMETRY_TOPIC,
					activeModule.node_id
				) ?? null)
			: null
	);

	let masterVolume = $derived(
		activeModule ? soundCardNodeAtPath(nodes, activeModule, 'parameters/master_volume_db') : null
	);
	let virtualInputs = $derived(
		activeModule ? soundCardNodeAtPath(nodes, activeModule, 'parameters/virtual_inputs') : null
	);
	let virtualOutputs = $derived(
		activeModule ? soundCardNodeAtPath(nodes, activeModule, 'parameters/virtual_outputs') : null
	);
	let inputProfiles = $derived(
		activeModule
			? soundCardNodeAtPath(nodes, activeModule, 'parameters/device_profiles/input_profiles')
			: null
	);
	let outputProfiles = $derived(
		activeModule
			? soundCardNodeAtPath(nodes, activeModule, 'parameters/device_profiles/output_profiles')
			: null
	);
	let monitoringRoutes = $derived(
		activeModule ? soundCardNodeAtPath(nodes, activeModule, 'parameters/monitoring_routes') : null
	);
	let playbackRoutes = $derived(
		activeModule ? soundCardNodeAtPath(nodes, activeModule, 'parameters/playback_routes') : null
	);
	let analysisNodes = $derived(
		activeModule ? soundCardNodeAtPath(nodes, activeModule, 'parameters/analysis') : null
	);
	let diagnosticNodes = $derived(
		activeModule ? soundCardNodeAtPath(nodes, activeModule, 'values/diagnostics') : null
	);

	let inputProfileNodes = $derived(
		soundCardDirectChildrenByType(nodes, inputProfiles, 'sound_card_input_profile')
	);
	let outputProfileNodes = $derived(
		soundCardDirectChildrenByType(nodes, outputProfiles, 'sound_card_output_profile')
	);
	let selectedInputProfile = $derived.by(() => {
		const requested = panelParams.inputProfileNodeId;
		const requestedProfile = requested === undefined ? null : nodes.get(requested);
		if (
			requestedProfile?.node_type === 'sound_card_input_profile' &&
			inputProfileNodes.some((profile) => profile.node_id === requestedProfile.node_id)
		) {
			return requestedProfile;
		}
		const activeKey = telemetry?.device.input.profile_key ?? '';
		return (
			inputProfileNodes.find((profile) => soundCardProfileKey(nodes, profile) === activeKey) ??
			inputProfileNodes[0] ??
			null
		);
	});
	let selectedOutputProfile = $derived.by(() => {
		const requested = panelParams.outputProfileNodeId;
		const requestedProfile = requested === undefined ? null : nodes.get(requested);
		if (
			requestedProfile?.node_type === 'sound_card_output_profile' &&
			outputProfileNodes.some((profile) => profile.node_id === requestedProfile.node_id)
		) {
			return requestedProfile;
		}
		const activeKey = telemetry?.device.output.profile_key ?? '';
		return (
			outputProfileNodes.find((profile) => soundCardProfileKey(nodes, profile) === activeKey) ??
			outputProfileNodes[0] ??
			null
		);
	});

	let inputPatchRows = $derived(
		soundCardRouteRecords(
			nodes,
			selectedInputProfile,
			'sound_card_input_patch_route',
			'physical_channel',
			'virtual_input'
		)
	);
	let outputPatchRows = $derived(
		soundCardRouteRecords(
			nodes,
			selectedOutputProfile,
			'sound_card_output_patch_route',
			'virtual_output',
			'physical_channel'
		)
	);
	let monitorRows = $derived(
		soundCardRouteRecords(
			nodes,
			monitoringRoutes,
			'sound_card_monitor_route',
			'virtual_input',
			'virtual_output'
		)
	);
	let playbackRows = $derived(
		soundCardRouteRecords(
			nodes,
			playbackRoutes,
			'sound_card_playback_route',
			'source_channel',
			'virtual_output'
		)
	);
	let virtualInputEndpoints = $derived(
		soundCardVirtualChannelEndpoints(nodes, virtualInputs, 'sound_card_virtual_input')
	);
	let virtualOutputEndpoints = $derived(
		soundCardVirtualChannelEndpoints(nodes, virtualOutputs, 'sound_card_virtual_output')
	);
	let physicalInputEndpoints = $derived(
		telemetry ? stablePhysicalEndpoints(telemetry.device, 'input') : []
	);
	let physicalOutputEndpoints = $derived(
		telemetry ? stablePhysicalEndpoints(telemetry.device, 'output') : []
	);
	let playbackSourceChannelLimit = $derived(telemetry?.playback_source_channel_limit ?? 0);
	let playbackSourceEndpoints = $derived(
		soundCardPlaybackSourceEndpoints(playbackSourceChannelLimit)
	);
	let channelLabels = $derived(soundCardChannelLabels(nodes, activeModule));
	let xruns = $derived(
		activeModule ? numericParameterAtPath(nodes, activeModule, 'values/diagnostics/xruns') : null
	);
	let lastError = $derived(
		activeModule
			? stringParameterAtPath(nodes, activeModule, 'values/diagnostics/last_error')
			: null
	);
	let inputActive = $derived(telemetry?.device.input.enabled ?? false);

	const panelTitle = (module: UiNodeDto): string =>
		resolveModuleEditor(module)?.title(module) ?? `Sound Card: ${module.meta.label}`;

	const updatePanelParams = (patch: Partial<EditorParams>): void => {
		const params = { ...panelState.params, ...patch };
		const next = { ...panelState, params };
		updatedPanelState = next;
		props.panelApi.updateParams(params);
	};

	$effect(() => {
		if (!activeModule) return;
		const title = panelTitle(activeModule);
		if (panelState.title !== title) props.panelApi.setTitle(title);
	});

	const selectModule = (event: Event): void => {
		const nodeId = Number((event.currentTarget as HTMLSelectElement).value);
		const module = nodes.get(nodeId);
		if (!module || module.node_type !== SOUND_CARD_MODULE_TYPE) return;
		const params = {
			...panelState.params,
			moduleNodeId: module.node_id,
			inputProfileNodeId: undefined,
			outputProfileNodeId: undefined
		};
		const next = { ...panelState, title: panelTitle(module), params };
		updatedPanelState = next;
		props.panelApi.updateParams(params);
		props.panelApi.setTitle(next.title);
	};

	const selectInputProfile = (event: Event): void => {
		const nodeId = Number((event.currentTarget as HTMLSelectElement).value);
		if (!inputProfileNodes.some((profile) => profile.node_id === nodeId)) return;
		updatePanelParams({ inputProfileNodeId: nodeId });
	};

	const selectOutputProfile = (event: Event): void => {
		const nodeId = Number((event.currentTarget as HTMLSelectElement).value);
		if (!outputProfileNodes.some((profile) => profile.node_id === nodeId)) return;
		updatePanelParams({ outputProfileNodeId: nodeId });
	};
</script>

<div class="sound-card-editor">
	<header class="editor-header">
		<div>
			<h1>Sound Card</h1>
			<p>Devices, authored routing, live signal, playback, analysis, and diagnostics.</p>
		</div>
		<label>
			<span>Module</span>
			<select value={String(activeModule?.node_id ?? '')} onchange={selectModule}>
				{#if soundCardModules.length === 0}
					<option value="">No Sound Card module</option>
				{/if}
				{#each soundCardModules as module (module.node_id)}
					<option value={String(module.node_id)}>{module.meta.label}</option>
				{/each}
			</select>
		</label>
	</header>

	{#if activeModule && binding}
		<main>
			<section class="editor-section" aria-labelledby="sound-card-devices-heading">
				<h2 id="sound-card-devices-heading">Devices</h2>
				<AudioDeviceSelector {binding} />
			</section>

			<section class="editor-section" aria-labelledby="sound-card-channels-heading">
				<h2 id="sound-card-channels-heading">Virtual channels</h2>
				<div class="section-grid">
					<SoundCardNodeSection title="Master output" node={masterVolume} />
					<SoundCardNodeSection title="Virtual inputs" node={virtualInputs} />
					<SoundCardNodeSection title="Virtual outputs" node={virtualOutputs} />
				</div>
				{#if telemetry}
					<SoundCardMeters inputs={telemetry.inputs} outputs={telemetry.outputs} {channelLabels} />
				{/if}
			</section>

			<section class="editor-section" aria-labelledby="sound-card-patch-heading">
				<h2 id="sound-card-patch-heading">Device patch</h2>
				<div class="profile-selectors">
					<label>
						<span>Input profile history</span>
						<select
							value={String(selectedInputProfile?.node_id ?? '')}
							onchange={selectInputProfile}>
							{#if inputProfileNodes.length === 0}
								<option value="">No input profile</option>
							{/if}
							{#each inputProfileNodes as profile (profile.node_id)}
								<option value={String(profile.node_id)}>
									{profile.meta.label}
									{soundCardProfileKey(nodes, profile) === telemetry?.device.input.profile_key
										? ' (active)'
										: ''}
								</option>
							{/each}
						</select>
					</label>
					<label>
						<span>Output profile history</span>
						<select
							value={String(selectedOutputProfile?.node_id ?? '')}
							onchange={selectOutputProfile}>
							{#if outputProfileNodes.length === 0}
								<option value="">No output profile</option>
							{/if}
							{#each outputProfileNodes as profile (profile.node_id)}
								<option value={String(profile.node_id)}>
									{profile.meta.label}
									{soundCardProfileKey(nodes, profile) === telemetry?.device.output.profile_key
										? ' (active)'
										: ''}
								</option>
							{/each}
						</select>
					</label>
				</div>
				<div class="matrix-grid">
					<SoundCardRouteMatrix
						title="Physical input → virtual input"
						rows={inputPatchRows}
						sources={physicalInputEndpoints}
						destinations={virtualInputEndpoints}
						parent={selectedInputProfile?.node_id ?? null}
						nodeType="sound_card_input_patch_route"
						sourceDeclId="physical_channel"
						destinationDeclId="virtual_input"
						sourceLabel="Physical input"
						destinationLabel="Virtual input"
						active={inputActive} />
					<SoundCardRouteMatrix
						title="Virtual output → physical output"
						rows={outputPatchRows}
						sources={virtualOutputEndpoints}
						destinations={physicalOutputEndpoints}
						parent={selectedOutputProfile?.node_id ?? null}
						nodeType="sound_card_output_patch_route"
						sourceDeclId="virtual_output"
						destinationDeclId="physical_channel"
						sourceLabel="Virtual output"
						destinationLabel="Physical output" />
				</div>
				<div class="section-grid">
					<SoundCardNodeSection
						title="Input profiles and routes"
						node={inputProfiles}
						open={false} />
					<SoundCardNodeSection
						title="Output profiles and routes"
						node={outputProfiles}
						open={false} />
				</div>
			</section>

			<section class="editor-section" aria-labelledby="sound-card-monitoring-heading">
				<h2 id="sound-card-monitoring-heading">Monitoring</h2>
				<SoundCardRouteMatrix
					title="Virtual input → virtual output"
					rows={monitorRows}
					sources={virtualInputEndpoints}
					destinations={virtualOutputEndpoints}
					parent={monitoringRoutes?.node_id ?? null}
					nodeType="sound_card_monitor_route"
					sourceDeclId="virtual_input"
					destinationDeclId="virtual_output"
					sourceLabel="Virtual input"
					destinationLabel="Virtual output"
					active={inputActive} />
				<SoundCardNodeSection
					title="Authored monitor routes"
					description="Routes remain authored while input is disabled."
					node={monitoringRoutes}
					open={false} />
			</section>

			<section class="editor-section" aria-labelledby="sound-card-playback-heading">
				<h2 id="sound-card-playback-heading">Playback</h2>
				<SoundCardRouteMatrix
					title="File source → virtual output"
					rows={playbackRows}
					sources={playbackSourceEndpoints}
					destinations={virtualOutputEndpoints}
					parent={playbackRoutes?.node_id ?? null}
					nodeType="sound_card_playback_route"
					sourceDeclId="source_channel"
					destinationDeclId="virtual_output"
					sourceLabel="File source"
					destinationLabel="Virtual output"
					emptyLabel="No playback patch routes are authored." />
				{#if telemetry}
					<SoundCardPlaybackStatus
						moduleNodeId={activeModule.node_id}
						activeCount={telemetry.playback.active_voices}
						loadingCount={telemetry.playback.loading_voices}
						voices={telemetry.playback_voices} />
				{/if}
				<SoundCardNodeSection title="Playback patch" node={playbackRoutes} open={false} />
			</section>

			<section class="editor-section" aria-labelledby="sound-card-analysis-heading">
				<h2 id="sound-card-analysis-heading">Analysis</h2>
				{#if telemetry}
					<SoundCardAnalysisView analysis={telemetry.analysis} />
				{:else}
					<p class="empty">Waiting for Sound Card analysis telemetry.</p>
				{/if}
				<SoundCardNodeSection title="Analysis configuration" node={analysisNodes} />
			</section>

			<section class="editor-section" aria-labelledby="sound-card-diagnostics-heading">
				<h2 id="sound-card-diagnostics-heading">Diagnostics</h2>
				{#if telemetry}
					<SoundCardDiagnostics {telemetry} {xruns} {lastError} />
				{:else}
					<p class="empty">Waiting for Sound Card diagnostics telemetry.</p>
				{/if}
				<SoundCardNodeSection
					title="Projected diagnostic values"
					node={diagnosticNodes}
					open={false} />
			</section>
		</main>
	{:else}
		<p class="missing">No Sound Card module found.</p>
	{/if}
</div>

<style>
	.sound-card-editor {
		display: flex;
		flex-direction: column;
		block-size: 100%;
		min-block-size: 0;
		background: var(--gc-color-bg);
		color: var(--gc-color-text);
	}

	.editor-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.8rem 1rem;
		border-block-end: 0.0625rem solid var(--gc-color-border);
		background: var(--gc-color-bg-light);
	}

	h1,
	h2,
	p {
		margin: 0;
	}

	h1 {
		font-size: 1.05rem;
	}

	h2 {
		font-size: 0.95rem;
	}

	.editor-header p {
		margin-block-start: 0.18rem;
		color: var(--gc-color-text-muted);
		font-size: 0.72rem;
	}

	.editor-header label {
		display: grid;
		gap: 0.2rem;
		min-inline-size: min(15rem, 42%);
		color: var(--gc-color-text-muted);
		font-size: 0.7rem;
	}

	select {
		min-block-size: 2rem;
		padding-inline: 0.45rem;
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.35rem;
		background: var(--gc-color-bg-lighter);
		color: var(--gc-color-text);
		font: inherit;
	}

	select:focus-visible {
		outline: 0.15rem solid var(--gc-color-accent);
		outline-offset: 0.1rem;
	}

	main {
		display: grid;
		gap: 0.9rem;
		min-block-size: 0;
		padding: 0.9rem;
		overflow: auto;
	}

	.editor-section {
		display: grid;
		gap: 0.7rem;
		padding: 0.8rem;
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.5rem;
		background: color-mix(in srgb, var(--gc-color-bg-light) 70%, transparent);
	}

	.section-grid,
	.matrix-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(19rem, 100%), 1fr));
		gap: 0.7rem;
	}

	.profile-selectors {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(15rem, 100%), 1fr));
		gap: 0.6rem;
	}

	.profile-selectors label {
		display: grid;
		gap: 0.2rem;
		color: var(--gc-color-text-muted);
		font-size: 0.7rem;
	}

	.empty,
	.missing {
		color: var(--gc-color-text-muted);
		font-size: 0.8rem;
	}

	.missing {
		margin: auto;
		padding: 1rem;
	}

	@media (max-width: 42rem) {
		.editor-header {
			align-items: stretch;
			flex-direction: column;
		}

		.editor-header label {
			min-inline-size: 0;
		}
	}
</style>
