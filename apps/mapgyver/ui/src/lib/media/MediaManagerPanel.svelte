<script lang="ts">
	import { untrack } from 'svelte';
	import { appState, type PanelProps, type PanelState } from 'golden_ui';
	import type { MediaEdit } from './generated/MediaEdit';
	import type { MediaEditResult } from './generated/MediaEditResult';
	import type { MediaManagerSnapshot } from './generated/MediaManagerSnapshot';
	import type { ChannelId } from './generated/ChannelId';
	import type { ChannelDefinition } from './generated/ChannelDefinition';
	import { invokeMediaEdit } from './operations';
	import SolidChannelEditor from './SolidChannelEditor.svelte';
	import CompositionChannelEditor from './CompositionChannelEditor.svelte';
	import PictureChannelEditor from './PictureChannelEditor.svelte';
	import SharedTextureChannelEditor from './SharedTextureChannelEditor.svelte';
	import AssetLocatorForm from './AssetLocatorForm.svelte';
	import ChannelPreview from './ChannelPreview.svelte';
	import { MEDIA_NAMESPACE } from './generated/constants';

	let { panelId, panelType, title, params }: PanelProps = $props();
	let panel = $state<PanelState | null>(null);
	export const setPanelState = (next: PanelState) => {
		panel = next;
	};
	const session = $derived(appState.session);
	let catalog = $state<MediaManagerSnapshot | null>(null);
	let selected = $state<ChannelId | null>(null);
	let pending = $state(false);
	let importing = $state(false);
	const busy = $derived(
		pending ||
			session?.status !== 'connected' ||
			session.loading.visible ||
			session.hasActiveEditSession
	);
	let error = $state('');
	let request = 0;
	const selectedMedia = $derived(
		catalog?.media.find((media) =>
			media.channels.some((channel) => channel.reference.channel === selected)
		)
	);
	const selectedChannel = $derived(
		selectedMedia?.channels.find((channel) => channel.reference.channel === selected)
	);

	async function refresh() {
		if (!session || session.status !== 'connected') return;
		// A live gesture publishes samples; inspect once when its shared edit group closes.
		if (session.hasActiveEditSession) return;
		const current = ++request;
		try {
			const result = await invokeMediaEdit(session, { kind: 'InspectManager' });
			if (current !== request || result.kind !== 'Manager') return;
			catalog = result.snapshot;
			if (
				!catalog.media.some((media) =>
					media.channels.some((channel) => channel.reference.channel === selected)
				)
			) {
				selected = catalog.media[0]?.channels[0]?.reference.channel ?? null;
			}
		} catch (cause) {
			if (current === request) error = String(cause);
		}
	}

	$effect(() => {
		// History and snapshot signals cover other panels, scripts, undo and project reload.
		// Inspection itself does not change either signal, so this never polls the engine.
		const current = session;
		if (current) {
			void current.currentHistoryStateId;
			void current.snapshotGeneration;
			void current.hasActiveEditSession;
			void current.editSessionEpoch;
			void current.status;
			void current.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		untrack(() => {
			void refresh();
		});
	});

	async function edit(intent: MediaEdit): Promise<MediaEditResult | undefined> {
		if (!session || busy) return;
		pending = true;
		error = '';
		try {
			const result = await invokeMediaEdit(session, intent);
			if (result.kind === 'Created' || result.kind === 'PictureImported')
				selected = result.reference.channel;
			await refresh();
			return result;
		} catch (cause) {
			error = String(cause);
		} finally {
			pending = false;
		}
	}

	async function configure(definition: ChannelDefinition) {
		if (selectedChannel)
			await edit({ kind: 'ConfigureChannel', reference: selectedChannel.reference, definition });
	}
</script>

<section
	class="manager"
	aria-label={panel?.title ?? title ?? 'Media Manager'}
	data-panel-id={panelId}
	data-panel-type={panelType}>
	<header>
		<div>
			<h2>Media Manager</h2>
			<p>Author reusable media and independent output channels.</p>
		</div>
		<button
			disabled={busy || session?.status !== 'connected'}
			onclick={() => edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label: null })}
			>Add Solid Color</button>
		<button
			disabled={busy}
			onclick={() => edit({ kind: 'CreateMedia', media_kind: 'Picture', label: null })}
			>Add Picture</button>
		<button
			disabled={busy}
			onclick={() => edit({ kind: 'CreateMedia', media_kind: 'SharedTexture', label: null })}
			>Add Shared Texture</button>
		<button
			disabled={busy}
			onclick={() => edit({ kind: 'CreateMedia', media_kind: 'Composition', label: null })}
			>Add Composition</button>
		<button
			disabled={busy}
			aria-expanded={importing}
			onclick={() => {
				importing = !importing;
			}}>Import Picture…</button>
		<button disabled={busy || session?.status !== 'connected'} onclick={refresh}>Refresh</button>
	</header>
	{#if importing}
		<AssetLocatorForm
			disabled={busy}
			submitLabel="Import Picture"
			submit={async (locator) => {
				if (await edit({ kind: 'ImportPicture', locator, target: null, label: null }))
					importing = false;
			}} />
	{/if}
	{#if error}<p class="error" role="alert">{error}</p>{/if}
	{#if session?.status !== 'connected'}
		<p class="empty">Connect to MapGyver to edit media.</p>
	{:else if catalog === null}
		<p class="empty">Loading media…</p>
	{:else if catalog.media.length === 0}
		<div class="empty">
			<h3>No media yet</h3>
			<p>Add media, then configure its independent output channels.</p>
		</div>
	{:else}
		<div class="workspace">
			<nav aria-label="Media and channels">
				{#each catalog.media as media (media.id)}
					<article class:active={selectedMedia?.id === media.id}>
						<form
							class="name"
							onsubmit={(event) => {
								event.preventDefault();
								const label = new FormData(event.currentTarget).get('label') as string;
								void edit({ kind: 'RenameMedia', media: media.id, label });
							}}>
							<input
								aria-label={`Media name: ${media.label}`}
								name="label"
								value={media.label}
								required
								disabled={busy} />
							<button disabled={busy}>Rename</button>
						</form>
						<small
							>{media.media_kind} · {media.channels.length} channel{media.channels.length === 1
								? ''
								: 's'}</small>
						<ul>
							{#each media.channels as channel, index (channel.reference.channel)}
								<li>
									<button
										class="channel"
										aria-pressed={selected === channel.reference.channel}
										onclick={() => {
											selected = channel.reference.channel;
											error = '';
										}}>
										<span>{channel.label}</span><small
											>{channel.definition.size.kind === 'Fixed'
												? `${channel.definition.size.extent.width} × ${channel.definition.size.extent.height}`
												: 'Automatic'}</small>
									</button>
									<button
										title="Move channel up"
										aria-label={`Move ${channel.label} up`}
										disabled={busy || index === 0}
										onclick={() =>
											edit({
												kind: 'ReorderChannel',
												reference: channel.reference,
												after: media.channels[index - 2]?.reference.channel ?? null
											})}>↑</button>
									<button
										title="Move channel down"
										aria-label={`Move ${channel.label} down`}
										disabled={busy || index === media.channels.length - 1}
										onclick={() =>
											edit({
												kind: 'ReorderChannel',
												reference: channel.reference,
												after: media.channels[index + 1].reference.channel
											})}>↓</button>
								</li>
							{/each}
						</ul>
						<div class="actions">
							<button
								disabled={busy}
								onclick={() => edit({ kind: 'AddChannel', media: media.id, label: null })}
								>Add channel</button>
							<button
								class="danger"
								disabled={busy}
								onclick={() => edit({ kind: 'RemoveMedia', media: media.id })}>Delete media</button>
						</div>
					</article>
				{/each}
			</nav>
			<div class="editor">
				{#if selectedChannel && selectedMedia}
					<ChannelPreview channel={selectedChannel} />
					<form
						class="name"
						onsubmit={(event) => {
							event.preventDefault();
							const label = new FormData(event.currentTarget).get('label') as string;
							void edit({ kind: 'RenameChannel', reference: selectedChannel!.reference, label });
						}}>
						<input
							aria-label="Channel name"
							name="label"
							value={selectedChannel.label}
							required
							disabled={busy} />
						<button disabled={busy}>Rename channel</button>
					</form>
					{#if selectedChannel.definition.source.kind === 'SolidColor'}
						<SolidChannelEditor channel={selectedChannel} disabled={busy} {configure} />
					{:else if selectedChannel.definition.source.kind === 'Picture'}
						<PictureChannelEditor
							channel={selectedChannel}
							assets={catalog.assets}
							disabled={busy}
							{configure}
							{edit} />
					{:else if selectedChannel.definition.source.kind === 'Spout'}
						<SharedTextureChannelEditor channel={selectedChannel} disabled={busy} {configure} />
					{:else if selectedChannel.definition.source.kind === 'Composition'}
						<CompositionChannelEditor channel={selectedChannel} disabled={busy} {configure} />
					{/if}
					<button
						class="danger"
						disabled={busy || selectedMedia.channels.length === 1}
						onclick={() => edit({ kind: 'RemoveChannel', reference: selectedChannel!.reference })}
						>Delete channel</button>
					{#if selectedMedia.channels.length === 1}<small>Media retains at least one channel.</small
						>{/if}
				{/if}
			</div>
		</div>
	{/if}
</section>

<style>
	.manager {
		container-type: inline-size;
		height: 100%;
		overflow: auto;
		padding: 1.2rem;
		box-sizing: border-box;
		color: #dedfe5;
		background: var(--mapgyver-panel-background, #181b22);
		font-size: 0.9rem;
	}
	header {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.7rem;
		margin-bottom: 1.2rem;
	}
	header > div {
		flex: 1 1 100%;
	}
	h2 {
		font-size: 1.2rem;
		margin: 0 0 0.35rem;
	}
	p {
		margin: 0;
	}
	header p,
	small {
		opacity: 0.65;
	}
	button,
	input {
		font: inherit;
		color: inherit;
		background: #ffffff0a;
		border: 0.0625rem solid #ffffff24;
		border-radius: 0.3rem;
		padding: 0.5em 0.65em;
		min-width: 0;
	}
	button {
		cursor: pointer;
	}
	button:hover {
		background: #ffffff18;
	}
	button:disabled {
		opacity: 0.4;
		cursor: default;
	}
	.workspace {
		display: grid;
		grid-template-columns: minmax(16rem, 1fr) minmax(16rem, 1.3fr);
		gap: 1.2rem;
	}
	nav {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		min-width: 0;
		align-content: start;
		gap: 0.8rem;
	}
	article {
		min-width: 0;
		border: 0.0625rem solid #ffffff18;
		border-radius: 0.5rem;
		padding: 0.8rem;
	}
	article.active {
		border-color: #648ac0;
	}
	.name {
		display: flex;
		gap: 0.5rem;
		margin-bottom: 0.6rem;
	}
	.name input {
		flex: 1;
		width: 0;
	}
	ul {
		list-style: none;
		padding: 0;
		margin: 0.8rem 0;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: 0.4rem;
	}
	li {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 0.3rem;
	}
	.channel {
		grid-column: 1 / -1;
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 0.7rem;
		text-align: left;
	}
	.channel span {
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.channel[aria-pressed='true'] {
		background: #416caa50;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}
	.editor {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		min-width: 0;
	}
	.editor > button {
		align-self: start;
	}
	.danger {
		color: #f5a3a3;
	}
	.error {
		padding: 0.8rem;
		margin-bottom: 1rem;
		color: #ffc1c1;
		background: #792d2d40;
		border-radius: 0.4rem;
		overflow-wrap: anywhere;
	}
	.empty {
		padding: 2rem;
		text-align: center;
		opacity: 0.7;
	}
	@container (max-width: 38rem) {
		.workspace {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
