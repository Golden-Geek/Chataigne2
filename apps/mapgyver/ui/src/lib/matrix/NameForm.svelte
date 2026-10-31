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

<form
	class="matrix-authoring"
	onsubmit={(event) => {
		event.preventDefault();
		rename(String(new FormData(event.currentTarget).get('label')));
	}}>
	<fieldset {disabled}>
		<legend>{label}</legend>
		<input name="label" {value} required aria-label={label} /><button>Rename</button>
	</fieldset>
</form>
