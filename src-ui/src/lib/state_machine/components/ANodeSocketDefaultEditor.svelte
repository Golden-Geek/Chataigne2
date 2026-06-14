<script lang="ts">
	import { resolveParameterEditor, type UiNodeDto } from 'golden_ui';

	let { parameter }: { parameter: UiNodeDto } = $props();

	let editorInfo = $derived(resolveParameterEditor(parameter, null));
	let EditorComponent = $derived(editorInfo?.component ?? null);
	let presentation = $derived.by(() => {
		const value =
			parameter.data.kind === 'parameter' ? parameter.data.param.value : null;
		switch (value?.kind) {
			case 'vec2':
			case 'vec3':
				return { layout: 'inline', show_value_fields: true, max_decimals: 2 };
			case 'int':
				return { show_value_field: true, max_decimals: 0 };
			case 'float':
				return { show_value_field: true, max_decimals: 3 };
			default:
				return {};
		}
	});
</script>

{#if EditorComponent}
	<div class="socket-default-editor">
		<EditorComponent node={parameter} layoutMode="widget" {presentation} />
	</div>
{/if}

<style>
	.socket-default-editor {
		display: flex;
		align-items: center;
		inline-size: 100%;
		min-inline-size: 0;
		font-size: 0.62rem;
	}

	.socket-default-editor :global(input),
	.socket-default-editor :global(select),
	.socket-default-editor :global(button) {
		min-block-size: 1.05rem;
		font-size: 0.62rem;
	}

	.socket-default-editor :global(.number-property-container),
	.socket-default-editor :global(.text-input-container),
	.socket-default-editor :global(.checkbox-editor),
	.socket-default-editor :global(.multi-number-container),
	.socket-default-editor :global(.color-picker-editor) {
		inline-size: 100%;
		min-inline-size: 0;
	}
</style>
