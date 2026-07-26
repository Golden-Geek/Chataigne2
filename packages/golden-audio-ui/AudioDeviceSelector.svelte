<script lang="ts">
	import { onDestroy } from 'svelte';
	import type {
		AudioBackendState,
		AudioBackendStatus,
		AudioBufferPolicy,
		AudioDeviceReadiness,
		AudioDirection,
		AudioRecoveryPolicy,
		AudioStreamStatus
	} from './generated';
	import {
		audioDeviceOptionGroups,
		audioDeviceTargetKey,
		findAudioDeviceTarget
	} from './device-options';
	import { AudioInspectorInteractionCoordinator } from './interaction';
	import { selectAudioDirectionTarget, setAudioDirectionEnabled } from './selector-actions';
	import type { AudioDeviceInspectorBinding } from './types';

	let { binding, showSettings = true } = $props<{
		binding: AudioDeviceInspectorBinding;
		showSettings?: boolean;
	}>();

	const interaction = new AudioInspectorInteractionCoordinator();
	let interactionMessage = $state<string | null>(null);
	let refreshing = $state(false);
	let pending = $state({
		inputEnabled: false,
		inputTarget: false,
		outputEnabled: false,
		outputTarget: false,
		recoveryPolicy: false,
		sampleRate: false,
		bufferPolicy: false,
		fixedBufferFrames: false
	});

	let deviceState = $derived(binding.state);
	let inputGroups = $derived(audioDeviceOptionGroups(deviceState, 'input'));
	let outputGroups = $derived(audioDeviceOptionGroups(deviceState, 'output'));
	let fixedBufferFrames = $derived(
		deviceState.buffer_policy.kind === 'fixed'
			? deviceState.buffer_policy.frames
			: (binding.fixedBufferFrames ?? null)
	);

	onDestroy(() => interaction.dispose());

	const readinessLabels: Record<AudioDeviceReadiness, string> = {
		disabled: 'Disabled',
		discovering: 'Discovering',
		missing: 'Missing',
		unavailable: 'Unavailable',
		busy: 'Busy',
		permission_denied: 'Permission denied',
		preparing: 'Preparing',
		primed: 'Primed',
		switching: 'Switching',
		recovering: 'Recovering',
		ready: 'Ready',
		failed: 'Failed'
	};

	const backendStateLabels: Record<AudioBackendState, string> = {
		compiled: 'Compiled',
		available: 'Available',
		unavailable: 'Unavailable',
		missing_server: 'Missing server',
		missing_driver: 'Missing driver',
		failed: 'Failed'
	};

	const humanize = (value: string): string =>
		value
			.split('_')
			.map((part) => `${part.slice(0, 1).toUpperCase()}${part.slice(1)}`)
			.join(' ');

	const backendStateLabel = (state: AudioBackendState): string => backendStateLabels[state];

	const selectedTargetLabel = (stream: AudioStreamStatus): string =>
		stream.selected_label ??
		(stream.selected_target?.kind === 'system_default' ? 'System Default' : 'No device selected');

	const activeTargetLabel = (stream: AudioStreamStatus): string => {
		if (!stream.active_target) return 'None';
		if (
			stream.selected_target &&
			audioDeviceTargetKey(stream.active_target) === audioDeviceTargetKey(stream.selected_target)
		) {
			return selectedTargetLabel(stream);
		}
		return stream.active_target.kind === 'system_default'
			? `System Default (${stream.active_target.backend})`
			: stream.active_target.device;
	};

	const activeBackendLabel = (stream: AudioStreamStatus): string =>
		deviceState.backends.find(
			(backend: AudioBackendStatus) => backend.backend === stream.active_target?.backend
		)?.label ??
		stream.active_target?.backend ??
		'None';

	const applyOutcome = (outcome: {
		readonly ignored: boolean;
		readonly message: string | null;
	}): void => {
		if (!outcome.ignored) interactionMessage = outcome.message;
	};

	const setEnabled = async (direction: AudioDirection, event: Event): Promise<void> => {
		const input = event.currentTarget as HTMLInputElement;
		const previous = direction === 'input' ? deviceState.input.enabled : deviceState.output.enabled;
		const key = direction === 'input' ? 'inputEnabled' : 'outputEnabled';
		pending[key] = true;
		const outcome = await interaction.submit(
			() => setAudioDirectionEnabled(binding, direction, input.checked),
			() => {
				input.checked = previous;
			}
		);
		pending[key] = false;
		applyOutcome(outcome);
	};

	const selectTarget = async (direction: AudioDirection, event: Event): Promise<void> => {
		const select = event.currentTarget as HTMLSelectElement;
		const stream = direction === 'input' ? deviceState.input : deviceState.output;
		const groups = direction === 'input' ? inputGroups : outputGroups;
		const previous = audioDeviceTargetKey(stream.selected_target);
		const target = findAudioDeviceTarget(groups, select.value);
		if (!target) {
			select.value = previous;
			interactionMessage = 'The selected audio device is no longer available.';
			return;
		}
		const key = direction === 'input' ? 'inputTarget' : 'outputTarget';
		pending[key] = true;
		const outcome = await interaction.submit(
			() => selectAudioDirectionTarget(binding, direction, target),
			() => {
				select.value = previous;
			}
		);
		pending[key] = false;
		applyOutcome(outcome);
	};

	const selectRecoveryPolicy = async (event: Event): Promise<void> => {
		const select = event.currentTarget as HTMLSelectElement;
		const previous = deviceState.output.recovery_policy;
		const policy = select.value as AudioRecoveryPolicy;
		pending.recoveryPolicy = true;
		const outcome = await interaction.submit(
			() => binding.setRecoveryPolicy(policy),
			() => {
				select.value = previous;
			}
		);
		pending.recoveryPolicy = false;
		applyOutcome(outcome);
	};

	const setSampleRate = async (event: Event): Promise<void> => {
		const input = event.currentTarget as HTMLInputElement;
		const previous = String(deviceState.engine_sample_rate);
		const rate = Number(input.value);
		if (!Number.isFinite(rate)) {
			input.value = previous;
			return;
		}
		pending.sampleRate = true;
		const outcome = await interaction.submit(
			() => binding.setSampleRate(rate),
			() => {
				input.value = previous;
			}
		);
		pending.sampleRate = false;
		applyOutcome(outcome);
	};

	const selectBufferPolicy = async (event: Event): Promise<void> => {
		const select = event.currentTarget as HTMLSelectElement;
		const previous = deviceState.buffer_policy.kind;
		const policy: AudioBufferPolicy =
			select.value === 'automatic'
				? { kind: 'automatic' }
				: fixedBufferFrames !== null
					? { kind: 'fixed', frames: fixedBufferFrames }
					: { kind: 'automatic' };
		if (select.value === 'fixed' && policy.kind !== 'fixed') {
			select.value = previous;
			interactionMessage = 'This application does not expose a persisted fixed buffer size.';
			return;
		}
		pending.bufferPolicy = true;
		const outcome = await interaction.submit(
			() => binding.setBufferPolicy(policy),
			() => {
				select.value = previous;
			}
		);
		pending.bufferPolicy = false;
		applyOutcome(outcome);
	};

	const setFixedBufferFrames = async (event: Event): Promise<void> => {
		const input = event.currentTarget as HTMLInputElement;
		const previous = String(fixedBufferFrames ?? '');
		const frames = Number(input.value);
		if (!Number.isFinite(frames)) {
			input.value = previous;
			return;
		}
		pending.fixedBufferFrames = true;
		const outcome = await interaction.submit(
			() => binding.setFixedBufferFrames(frames),
			() => {
				input.value = previous;
			}
		);
		pending.fixedBufferFrames = false;
		applyOutcome(outcome);
	};

	const refreshDevices = async (): Promise<void> => {
		refreshing = true;
		interactionMessage = null;
		await interaction.refresh(binding.refreshDevices.bind(binding), (message) => {
			refreshing = false;
			interactionMessage = message;
		});
	};

	const copyTechnicalDetail = async (detail: string): Promise<void> => {
		if (typeof navigator === 'undefined' || !navigator.clipboard) return;
		await navigator.clipboard.writeText(detail);
	};
</script>

{#snippet streamCard(direction: AudioDirection, stream: AudioStreamStatus)}
	{@const groups = direction === 'input' ? inputGroups : outputGroups}
	{@const enabledPending = direction === 'input' ? pending.inputEnabled : pending.outputEnabled}
	{@const targetPending = direction === 'input' ? pending.inputTarget : pending.outputTarget}
	<section class="stream-card" aria-label="{humanize(direction)} audio stream">
		<header>
			<div>
				<h3>{humanize(direction)}</h3>
				<p>{selectedTargetLabel(stream)}</p>
			</div>
			<label class="enable-control">
				<input
					type="checkbox"
					checked={stream.enabled}
					disabled={enabledPending}
					onchange={(event) => void setEnabled(direction, event)} />
				<span>Enabled</span>
			</label>
		</header>

		<label class="field">
			<span>{humanize(direction)} device</span>
			<select
				value={audioDeviceTargetKey(stream.selected_target)}
				disabled={targetPending || deviceState.discovery_in_progress}
				onchange={(event) => void selectTarget(direction, event)}>
				{#if groups.length > 0 && !stream.selected_target}
					<option value="" disabled>No device selected</option>
				{/if}
				{#if groups.length === 0}
					<option value="">No audio backends discovered</option>
				{/if}
				{#each groups as group (group.backend + group.backendLabel)}
					<optgroup label="{group.backendLabel} — {backendStateLabels[group.backendState]}">
						{#each group.options as option (option.key)}
							<option value={option.key}>
								{option.label}{option.missing ? ' — Missing' : ''}
							</option>
						{/each}
					</optgroup>
				{/each}
			</select>
		</label>

		<div class="stream-status readiness-{stream.readiness}" role="status" aria-live="polite">
			<strong>{readinessLabels[stream.readiness]}</strong>
			<span>Permission: {humanize(stream.permission)}</span>
			{#if stream.retry_attempt > 0}
				<span>
					Retry {stream.retry_attempt}{stream.next_retry_ms !== null
						? ` in ${stream.next_retry_ms} ms`
						: ''}
				</span>
			{/if}
		</div>

		<dl class="stream-summary">
			<div>
				<dt>Active device</dt>
				<dd>{activeTargetLabel(stream)}</dd>
			</div>
			<div>
				<dt>Active backend</dt>
				<dd>{activeBackendLabel(stream)}</dd>
			</div>
			{#if stream.format}
				<div>
					<dt>Format</dt>
					<dd>
						{stream.format.sample_rate.toLocaleString()} Hz · {stream.format.channels}
						ch · {stream.format.sample_format}
					</dd>
				</div>
				<div>
					<dt>Buffer</dt>
					<dd>
						{stream.format.buffer_frames} frames ·
						{stream.format.estimated_latency_ms.toFixed(1)} ms estimated
					</dd>
				</div>
			{/if}
		</dl>

		{#if stream.error}
			<details class="diagnostic">
				<summary>{humanize(stream.error.category)}</summary>
				<p>{stream.error.message}</p>
				{#if stream.error.technical_detail}
					<pre>{stream.error.technical_detail}</pre>
					<button
						type="button"
						onclick={() => void copyTechnicalDetail(stream.error?.technical_detail ?? '')}>
						Copy technical detail
					</button>
				{/if}
			</details>
		{/if}
	</section>
{/snippet}

<section class="audio-device-selector" aria-label="Audio devices">
	<header class="selector-header">
		<div>
			<h2>Audio devices</h2>
			<p>
				{deviceState.discovery_in_progress
					? 'Discovering audio devices…'
					: `${deviceState.devices.length} device${deviceState.devices.length === 1 ? '' : 's'} discovered`}
			</p>
		</div>
		<button
			type="button"
			disabled={refreshing || deviceState.discovery_in_progress}
			onclick={() => void refreshDevices()}>
			{refreshing || deviceState.discovery_in_progress ? 'Refreshing…' : 'Refresh'}
		</button>
	</header>

	{#if deviceState.backends.length > 0}
		<ul class="backend-statuses" aria-label="Audio backend status">
			{#each deviceState.backends as backend (backend.backend)}
				<li class="backend-{backend.state}">
					<span>{backend.label}</span>
					<strong>{backendStateLabel(backend.state)}</strong>
					{#if backend.detail}
						<small>{backend.detail}</small>
					{/if}
				</li>
			{/each}
		</ul>
	{:else if !deviceState.discovery_in_progress}
		<p class="empty-state">No audio backend is available.</p>
	{/if}

	<div class="stream-grid">
		{@render streamCard('input', deviceState.input)}
		{@render streamCard('output', deviceState.output)}
	</div>

	{#if showSettings}
		<section class="settings" aria-labelledby="audio-engine-settings-heading">
			<h3 id="audio-engine-settings-heading">Engine settings</h3>
			<div class="settings-grid">
				<label class="field">
					<span>Recovery policy</span>
					<select
						value={deviceState.output.recovery_policy}
						disabled={pending.recoveryPolicy}
						onchange={(event) => void selectRecoveryPolicy(event)}>
						<option value="wait_for_selected">Wait for selected device</option>
						<option value="follow_system_default">Follow system default</option>
					</select>
				</label>

				<label class="field">
					<span>Engine sample rate</span>
					<input
						type="number"
						value={deviceState.engine_sample_rate}
						disabled={pending.sampleRate}
						inputmode="numeric"
						onchange={(event) => void setSampleRate(event)} />
				</label>

				<label class="field">
					<span>Buffer policy</span>
					<select
						value={deviceState.buffer_policy.kind}
						disabled={pending.bufferPolicy}
						onchange={(event) => void selectBufferPolicy(event)}>
						<option value="automatic">Automatic</option>
						<option value="fixed" disabled={fixedBufferFrames === null}>Fixed</option>
					</select>
				</label>

				{#if deviceState.buffer_policy.kind === 'fixed'}
					<label class="field">
						<span>Fixed buffer frames</span>
						<input
							type="number"
							value={fixedBufferFrames ?? deviceState.buffer_policy.frames}
							disabled={pending.fixedBufferFrames}
							inputmode="numeric"
							onchange={(event) => void setFixedBufferFrames(event)} />
					</label>
				{/if}
			</div>
		</section>
	{/if}

	{#if interactionMessage}
		<p class="interaction-error" role="alert" aria-live="assertive">
			{interactionMessage}
		</p>
	{/if}
</section>

<style>
	.audio-device-selector {
		display: grid;
		gap: 0.85rem;
		min-inline-size: 0;
		padding: 0.75rem;
		color: var(--audio-ui-text, #e8edf5);
	}

	.selector-header,
	.stream-card > header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
	}

	h2,
	h3,
	p {
		margin: 0;
	}

	h2 {
		font-size: 1rem;
	}

	h3 {
		font-size: 0.92rem;
	}

	.selector-header p,
	.stream-card header p {
		margin-block-start: 0.15rem;
		color: var(--audio-ui-muted, #aab4c4);
		font-size: 0.78rem;
	}

	button,
	select,
	input {
		min-block-size: 2rem;
		border: 0.0625rem solid var(--audio-ui-border, #465065);
		border-radius: 0.35rem;
		background: var(--audio-ui-control, #202737);
		color: inherit;
		font: inherit;
	}

	button {
		padding-inline: 0.75rem;
		cursor: pointer;
	}

	button:disabled,
	select:disabled,
	input:disabled {
		cursor: default;
		opacity: 0.55;
	}

	button:focus-visible,
	select:focus-visible,
	input:focus-visible,
	summary:focus-visible {
		outline: 0.15rem solid var(--audio-ui-focus, #70b7ff);
		outline-offset: 0.12rem;
	}

	.backend-statuses {
		display: grid;
		gap: 0.35rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.backend-statuses li {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 0.15rem 0.6rem;
		padding: 0.45rem 0.55rem;
		border-inline-start: 0.2rem solid var(--audio-ui-status, #7e899c);
		background: color-mix(in srgb, var(--audio-ui-control, #202737) 82%, transparent);
		font-size: 0.78rem;
	}

	.backend-statuses small {
		grid-column: 1 / -1;
		color: var(--audio-ui-muted, #aab4c4);
	}

	.backend-available {
		--audio-ui-status: #58c98c;
	}

	.backend-missing_server,
	.backend-missing_driver,
	.backend-unavailable {
		--audio-ui-status: #e3aa50;
	}

	.backend-failed {
		--audio-ui-status: #f17676;
	}

	.stream-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(17rem, 100%), 1fr));
		gap: 0.7rem;
	}

	.stream-card,
	.settings {
		display: grid;
		align-content: start;
		gap: 0.65rem;
		padding: 0.7rem;
		border: 0.0625rem solid var(--audio-ui-border, #465065);
		border-radius: 0.45rem;
		background: color-mix(in srgb, var(--audio-ui-control, #202737) 55%, transparent);
	}

	.enable-control {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		font-size: 0.8rem;
	}

	.enable-control input {
		min-block-size: auto;
	}

	.field {
		display: grid;
		gap: 0.25rem;
		min-inline-size: 0;
		color: var(--audio-ui-muted, #aab4c4);
		font-size: 0.75rem;
	}

	.field select,
	.field input {
		inline-size: 100%;
		min-inline-size: 0;
		padding-inline: 0.45rem;
		color: var(--audio-ui-text, #e8edf5);
	}

	.stream-status {
		display: flex;
		flex-wrap: wrap;
		gap: 0.35rem 0.7rem;
		padding: 0.4rem 0.5rem;
		border-inline-start: 0.2rem solid var(--audio-ui-status, #7e899c);
		background: color-mix(in srgb, var(--audio-ui-control, #202737) 82%, transparent);
		font-size: 0.76rem;
	}

	.readiness-ready,
	.readiness-primed {
		--audio-ui-status: #58c98c;
	}

	.readiness-missing,
	.readiness-unavailable,
	.readiness-busy,
	.readiness-recovering,
	.readiness-discovering {
		--audio-ui-status: #e3aa50;
	}

	.readiness-permission_denied,
	.readiness-failed {
		--audio-ui-status: #f17676;
	}

	.stream-summary {
		display: grid;
		gap: 0.35rem;
		margin: 0;
	}

	.stream-summary div {
		display: grid;
		grid-template-columns: minmax(6rem, 0.7fr) minmax(0, 1.3fr);
		gap: 0.5rem;
		font-size: 0.76rem;
	}

	.stream-summary dt {
		color: var(--audio-ui-muted, #aab4c4);
	}

	.stream-summary dd {
		min-inline-size: 0;
		margin: 0;
		overflow-wrap: anywhere;
	}

	.diagnostic {
		font-size: 0.76rem;
	}

	.diagnostic summary {
		cursor: pointer;
		color: #f6c87b;
	}

	.diagnostic p,
	.diagnostic pre {
		margin-block-start: 0.45rem;
	}

	.diagnostic pre {
		max-block-size: 12rem;
		overflow: auto;
		padding: 0.5rem;
		white-space: pre-wrap;
		background: #111722;
		color: #d7deea;
	}

	.settings-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(12rem, 100%), 1fr));
		gap: 0.6rem;
	}

	.empty-state,
	.interaction-error {
		padding: 0.55rem;
		border-radius: 0.35rem;
		font-size: 0.8rem;
	}

	.empty-state {
		color: var(--audio-ui-muted, #aab4c4);
		background: color-mix(in srgb, var(--audio-ui-control, #202737) 65%, transparent);
	}

	.interaction-error {
		border: 0.0625rem solid #8e4444;
		background: #3c2328;
		color: #ffd8d8;
	}
</style>
