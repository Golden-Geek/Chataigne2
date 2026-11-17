<script lang="ts">
	import { tick } from 'svelte';
	import { selectOnPress } from 'golden_ui';
	let {
		label,
		disabled,
		select,
		rename
	}: {
		label: string;
		disabled: boolean;
		select(event: PointerEvent | MouseEvent | KeyboardEvent): void;
		rename(value: string): Promise<boolean>;
	} = $props();
	let editing = $state(false),
		draft = $state(''),
		committing = $state(false);
	let input = $state<HTMLInputElement>();
	async function start(event: MouseEvent) {
		event.stopPropagation();
		if (disabled) return;
		draft = label;
		editing = true;
		await tick();
		input?.focus();
		input?.select();
	}
	async function finish() {
		if (!editing || committing) return;
		if (draft === label) {
			editing = false;
			return;
		}
		committing = true;
		try {
			if (await rename(draft)) editing = false;
		} finally {
			committing = false;
		}
	}
</script>

{#if editing}
	<input
		bind:this={input}
		bind:value={draft}
		aria-label={`Rename ${label}`}
		disabled={disabled || committing}
		onblur={() => void finish()}
		onkeydown={(event) => {
			if (event.key === 'Escape') {
				event.preventDefault();
				editing = false;
			}
			if (event.key === 'Enter') {
				event.preventDefault();
				void finish();
			}
		}} />
{:else}
	<button
		{disabled}
		use:selectOnPress={{ select, disabled }}
		title={`Select ${label}`}
		ondblclick={start}>{label}</button>
{/if}

<style>
	button,
	input {
		min-width: 0;
		max-width: 100%;
		width: 100%;
		box-sizing: border-box;
		text-overflow: ellipsis;
		overflow: hidden;
		white-space: nowrap;
	}
	button {
		text-align: left;
	}
</style>
