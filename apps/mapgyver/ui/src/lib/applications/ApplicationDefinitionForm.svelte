<script lang="ts">
	import { untrack } from 'svelte';
	import type { ApplicationDefinition } from '../media/generated/ApplicationDefinition';
	import type { ApplicationReadiness } from '../media/generated/ApplicationReadiness';
	import SpoutSenderPicker from '../media/SpoutSenderPicker.svelte';
	import { definitionFromForm } from './definition-form';
	let {
		definition,
		disabled,
		save
	}: {
		definition: ApplicationDefinition;
		disabled: boolean;
		save(value: ApplicationDefinition): unknown;
	} = $props();
	let argumentsDraft = $state(untrack(() => [...definition.arguments]));
	let environment = $state(
		untrack(() => definition.environment?.map(([name, value]) => ({ name, value })) ?? [])
	);
	let inherit = $state(untrack(() => definition.environment === null));
	let readiness = $state<ApplicationReadiness['kind']>(untrack(() => definition.readiness.kind));
	let sender = $state(
		untrack(() => (definition.readiness.kind === 'Spout' ? definition.readiness.sender : ''))
	);
	let error = $state('');
</script>

<form
	onsubmit={(event) => {
		event.preventDefault();
		error = '';
		try {
			void save(
				definitionFromForm(new FormData(event.currentTarget), definition, readiness, sender)
			);
		} catch (cause) {
			error = String(cause);
		}
	}}>
	<fieldset {disabled}>
		<legend>Launch definition</legend>
		<label>Executable path<input name="executable" value={definition.executable ?? ''} /></label>
		<label>Working directory<input name="directory" value={definition.directory ?? ''} /></label>
		<p>
			Leave the executable empty to keep this shared definition unconfigured. Arguments are separate
			values; shell quoting is not required.
		</p>
		{#each argumentsDraft as argument, index}
			<div class="row">
				<input
					name="argument"
					aria-label={`Argument ${index + 1}`}
					value={argument}
					oninput={(event) => {
						argumentsDraft[index] = event.currentTarget.value;
					}} />
				<button
					type="button"
					onclick={() => {
						argumentsDraft = argumentsDraft.filter((_, i) => i !== index);
					}}>Remove argument {index + 1}</button>
			</div>
		{/each}
		<button
			type="button"
			onclick={() => {
				argumentsDraft = [...argumentsDraft, ''];
			}}>Add argument</button>
		<label
			>Window<select name="window" value={definition.window}
				><option>Hidden</option><option>Normal</option></select
			></label>
	</fieldset>
	<fieldset {disabled}>
		<legend>Environment</legend>
		<label
			><input type="checkbox" name="inherit-environment" bind:checked={inherit} />Inherit the
			current environment</label>
		{#if !inherit}
			<p>This list replaces the environment. An empty list supplies an empty environment.</p>
			{#each environment as entry, index}
				<div class="row">
					<input
						name="environment-name"
						aria-label={`Environment name ${index + 1}`}
						bind:value={entry.name}
						required />
					<input
						name="environment-value"
						aria-label={`Environment value ${index + 1}`}
						bind:value={entry.value} />
					<button
						type="button"
						onclick={() => {
							environment = environment.filter((_, i) => i !== index);
						}}>Remove environment entry {index + 1}</button>
				</div>
			{/each}
			<button
				type="button"
				onclick={() => {
					environment = [...environment, { name: '', value: '' }];
				}}>Add environment entry</button>
		{/if}
	</fieldset>
	<fieldset {disabled}>
		<legend>Startup readiness</legend>
		<label
			>Ready when<select bind:value={readiness}
				><option value="ProcessStarted">Process started</option><option value="Spout"
					>Spout sender available</option
				></select
			></label>
		{#if readiness === 'Spout'}
			<SpoutSenderPicker bind:sender {disabled} />
			<label
				>Readiness evidence<select
					name="readiness-evidence"
					required
					value={definition.readiness.kind === 'Spout'
						? definition.readiness.first_frame
							? 'image'
							: 'sender'
						: ''}>
					<option value="" disabled>Choose readiness evidence</option><option value="sender"
						>Sender registration</option
					><option value="image">Received first image</option>
				</select></label>
			<label
				>Startup timeout (ms)<input
					name="readiness-timeout"
					type="number"
					min="1"
					max="600000"
					step="1"
					required
					value={definition.readiness.kind === 'Spout'
						? definition.readiness.timeout_ms
						: ''} /></label>
		{/if}
	</fieldset>
	<fieldset {disabled}>
		<legend>Stop and recovery</legend>
		<label
			>Graceful stop (ms)<input
				name="graceful-stop"
				type="number"
				min="0"
				max="600000"
				step="1"
				required
				value={definition.stop.graceful_ms} /></label>
		<label
			>Retirement timeout (ms)<input
				name="retirement-timeout"
				type="number"
				min="1"
				max="600000"
				step="1"
				required
				value={definition.stop.retirement_ms} /></label>
		<label
			><input name="restart" type="checkbox" checked={definition.restart.enabled} />Restart after
			failure</label>
		<label
			>Initial restart delay (ms)<input
				name="restart-initial"
				type="number"
				min="1"
				max="600000"
				step="1"
				required
				value={definition.restart.initial_ms} /></label>
		<label
			>Maximum restart delay (ms)<input
				name="restart-maximum"
				type="number"
				min="1"
				max="600000"
				step="1"
				required
				value={definition.restart.maximum_ms} /></label>
	</fieldset>
	{#if error}<p role="alert">{error}</p>{/if}
	<button {disabled}>Save application definition</button>
	<p>
		Saving does not launch an unloaded app. A running configuration remains retained while outgoing
		or final render use still needs it.
	</p>
</form>

<style>
	fieldset,
	label {
		display: grid;
		gap: 0.4rem;
		min-width: 0;
	}
	fieldset {
		margin: 0.75rem 0;
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.4rem;
	}
	input,
	select {
		min-width: 0;
		max-width: 100%;
	}
	p {
		font-size: 0.9em;
		opacity: 0.8;
	}
</style>
