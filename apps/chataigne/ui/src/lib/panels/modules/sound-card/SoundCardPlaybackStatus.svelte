<script lang="ts">
	import type { SoundCardPlaybackVoiceDto } from '$lib/modules/audio/sound-card/generated';
	import type { NodeId } from 'golden_ui';
	import { sendSoundCardPlaybackControl } from './sound-card-playback-controls';

	let { moduleNodeId, activeCount, loadingCount, voices } = $props<{
		moduleNodeId: NodeId;
		activeCount: number;
		loadingCount: number;
		voices: readonly SoundCardPlaybackVoiceDto[];
	}>();

	let pendingStops = $state<ReadonlySet<string>>(new Set());
	let stopAllPending = $state(false);
	let controlError = $state<string | null>(null);

	const setPlaybackPending = (playbackId: string, pending: boolean): void => {
		const next = new Set(pendingStops);
		if (pending) next.add(playbackId);
		else next.delete(playbackId);
		pendingStops = next;
	};

	const stopPlayback = async (playbackId: string): Promise<void> => {
		setPlaybackPending(playbackId, true);
		controlError = null;
		const success = await sendSoundCardPlaybackControl(moduleNodeId, {
			kind: 'stop_file',
			playback_id: playbackId
		});
		if (!success) {
			setPlaybackPending(playbackId, false);
			controlError = `Could not stop playback “${playbackId}”.`;
		}
	};

	const stopAll = async (): Promise<void> => {
		stopAllPending = true;
		controlError = null;
		const success = await sendSoundCardPlaybackControl(moduleNodeId, {
			kind: 'stop_all_files'
		});
		if (!success) {
			stopAllPending = false;
			controlError = 'Could not stop all Sound Card playback.';
		}
	};

	const fileName = (path: string): string => path.split(/[\\/]/).at(-1) || path;

	$effect(() => {
		const activeIds = new Set(voices.map((voice: SoundCardPlaybackVoiceDto) => voice.playback_id));
		const next = new Set([...pendingStops].filter((id) => activeIds.has(id)));
		if (next.size !== pendingStops.size) pendingStops = next;
		if (voices.length === 0) stopAllPending = false;
	});
</script>

<section class="playback-status" aria-labelledby="sound-card-playback-status-heading">
	<header>
		<div>
			<h3 id="sound-card-playback-status-heading">Playback lifecycle</h3>
			<p>{activeCount} active · {loadingCount} loading</p>
		</div>
		<button
			type="button"
			onclick={() => void stopAll()}
			disabled={voices.length === 0 || stopAllPending}>
			{stopAllPending ? 'Stopping…' : 'Stop all'}
		</button>
	</header>

	{#if voices.length > 0}
		<div class="voice-list">
			<table>
				<thead>
					<tr>
						<th scope="col">Playback</th>
						<th scope="col">File</th>
						<th scope="col">Voice</th>
						<th scope="col">State</th>
						<th scope="col"><span class="visually-hidden">Actions</span></th>
					</tr>
				</thead>
				<tbody>
					{#each voices as voice (voice.playback_id)}
						<tr>
							<td>{voice.playback_id}</td>
							<td title={voice.path}>{fileName(voice.path)}</td>
							<td>{voice.voice}</td>
							<td>{voice.lifecycle}</td>
							<td>
								<button
									type="button"
									onclick={() => void stopPlayback(voice.playback_id)}
									disabled={pendingStops.has(voice.playback_id)}>
									{pendingStops.has(voice.playback_id) ? 'Stopping…' : 'Stop'}
								</button>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{:else}
		<p class="empty">No active playback voices.</p>
	{/if}

	{#if controlError}
		<p class="control-error" role="alert">{controlError}</p>
	{/if}
</section>

<style>
	.playback-status {
		display: grid;
		gap: 0.55rem;
		padding: 0.65rem;
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.4rem;
		background: var(--gc-color-bg-light);
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.7rem;
	}

	h3,
	p {
		margin: 0;
	}

	h3 {
		font-size: 0.85rem;
	}

	header p,
	.empty {
		margin-block-start: 0.15rem;
		color: var(--gc-color-text-muted);
		font-size: 0.7rem;
	}

	button {
		min-block-size: 1.9rem;
		padding-inline: 0.65rem;
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.3rem;
		background: var(--gc-color-bg-lighter);
		color: var(--gc-color-text);
		font: inherit;
		cursor: pointer;
	}

	button:hover:not(:disabled) {
		border-color: var(--gc-color-accent);
	}

	button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.voice-list {
		max-block-size: 14rem;
		overflow: auto;
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.3rem;
	}

	table {
		inline-size: 100%;
		border-collapse: collapse;
		font-size: 0.72rem;
	}

	th,
	td {
		padding: 0.38rem 0.45rem;
		border-block-end: 0.0625rem solid var(--gc-color-border);
		text-align: start;
		overflow-wrap: anywhere;
	}

	th {
		position: sticky;
		inset-block-start: 0;
		background: var(--gc-color-bg-lighter);
		color: var(--gc-color-text-muted);
	}

	.control-error {
		padding: 0.5rem;
		border-radius: 0.3rem;
		background: #3c2328;
		color: #ffb4b4;
		font-size: 0.74rem;
	}

	.visually-hidden {
		position: absolute;
		inline-size: 0.0625rem;
		block-size: 0.0625rem;
		padding: 0;
		margin: -0.0625rem;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}
</style>
