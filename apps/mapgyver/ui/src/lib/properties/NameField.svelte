<script lang="ts">
	let {
		label,
		value,
		disabled,
		rename
	}: {
		label: string;
		value: string;
		disabled: boolean;
		rename(value: string): unknown;
	} = $props();
</script>

<label class="name-field">
	{label}
	<input
		{value}
		{disabled}
		required
		aria-label={label}
		onchange={(event) => rename(event.currentTarget.value)}
		onkeydown={(event) => {
			if (event.key === 'Enter') {
				event.preventDefault();
				event.currentTarget.blur();
			}
		}} />
</label>

<style>
	.name-field {
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}
	input {
		flex: 1;
		min-width: 0;
	}
</style>
