<script lang="ts">
	import previousIcon from '../icons/step-previous.svg';
	import nextIcon from '../icons/step-next.svg';
	import previousGroupIcon from '../icons/step-group-previous.svg';
	import nextGroupIcon from '../icons/step-group-next.svg';
	import ParameterTrigger from './ParameterTrigger.svelte';

	let { media, disabled }: { media: string; disabled: boolean } = $props();
	const controls = [
		{
			label: 'Columns',
			buttons: [
				{
					label: 'Previous Column',
					icon: previousIcon,
					field: 'previous_column'
				},
				{
					label: 'Next Column',
					icon: nextIcon,
					field: 'next_column'
				}
			]
		},
		{
			label: 'Top-level Column Groups',
			buttons: [
				{
					label: 'Previous Column Group',
					icon: previousGroupIcon,
					field: 'previous_column_group'
				},
				{
					label: 'Next Column Group',
					icon: nextGroupIcon,
					field: 'next_column_group'
				}
			]
		}
	] as const;
</script>

<div class="matrix-transport">
	{#each controls as group (group.label)}
		<div class="matrix-transport-pair" role="group" aria-label={group.label}>
			{#each group.buttons as control (control.label)}
				<ParameterTrigger owner={media} {...control} {disabled} tinted />
			{/each}
		</div>
	{/each}
	<ParameterTrigger
		owner={media}
		field="stop_all"
		label="Trigger Stop All"
		title="Stop All"
		{disabled}
		tinted />
</div>

<style>
	.matrix-transport {
		display: flex;
		align-items: center;
		gap: 0.35rem;
	}
	.matrix-transport-pair {
		display: flex;
		align-items: center;
		gap: 0.1rem;
		padding-inline-end: 0.35rem;
		border-inline-end: 0.0625rem solid var(--gc-color-border);
	}
</style>
