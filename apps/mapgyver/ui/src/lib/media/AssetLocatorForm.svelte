<script lang="ts">
	import { hasDesktopHost, openDesktopFileDialog } from 'golden_ui/host/desktop';
	import type { AssetLocator } from './generated/AssetLocator';

	let {
		initial = null,
		disabled,
		submitLabel,
		submit
	}: {
		initial?: AssetLocator | null;
		disabled: boolean;
		submitLabel: string;
		submit: (locator: AssetLocator) => Promise<unknown>;
	} = $props();
	let kind = $state<AssetLocator['kind'] | ''>('');
	let path = $state('');
	let browsing = $state(false);
	const initialKind = $derived(initial?.kind ?? '');
	const initialPath = $derived(initial?.path ?? '');
	$effect(() => {
		kind = initialKind;
		path = initialPath;
	});
	async function browse() {
		browsing = true;
		try {
			const selected = await openDesktopFileDialog({
				title: 'Choose a Picture',
				allowedExtensions: ['png', 'jpg', 'jpeg'],
				filterLabel: 'Pictures'
			});
			if (selected) {
				kind = 'absolute';
				path = selected;
			}
		} finally {
			browsing = false;
		}
	}
	async function apply(event: SubmitEvent) {
		event.preventDefault();
		if (kind) await submit({ kind, path });
	}
</script>

<form onsubmit={apply}>
	<fieldset disabled={disabled || browsing}>
		<label
			>Source location<select required bind:value={kind}>
				<option value="" disabled>Choose location type</option>
				<option value="project_relative">Relative to the project</option>
				<option value="absolute">Absolute path on this host</option>
			</select></label>
		<label>Picture file path<input required bind:value={path} /></label>
		<p>PNG or JPEG. Paths refer to files on the MapGyver host.</p>
		{#if kind === 'project_relative'}<p>
				Relative paths resolve from the saved project’s folder.
			</p>{/if}
		<div class="actions">
			{#if hasDesktopHost()}<button type="button" onclick={browse}>Browse…</button>{/if}
			<button type="submit">{submitLabel}</button>
		</div>
	</fieldset>
</form>

<style>
	fieldset {
		display: grid;
		gap: 0.7rem;
		border: 0.0625rem solid #ffffff24;
		border-radius: 0.5rem;
		padding: 1rem;
		min-width: 0;
	}
	label {
		display: grid;
		gap: 0.35em;
		min-width: 0;
	}
	input,
	select,
	button {
		font: inherit;
		color: inherit;
		background: #ffffff0a;
		border: 0.0625rem solid #ffffff30;
		border-radius: 0.3rem;
		padding: 0.5em 0.65em;
		min-width: 0;
	}
	button {
		cursor: pointer;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}
	p {
		margin: 0;
		opacity: 0.65;
		font-size: 0.9em;
	}
</style>
