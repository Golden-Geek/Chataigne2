<script lang="ts">
	import { resolveParameterEditor, type UiNodeDto } from 'golden_ui';

	let { parameter }: { parameter: UiNodeDto } = $props();

	let editorInfo = $derived(resolveParameterEditor(parameter, null));
	let EditorComponent = $derived(editorInfo?.component ?? null);
	let presentation = $derived.by(() => {
		const value = parameter.data.kind === 'parameter' ? parameter.data.param.value : null;
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
		<!-- Default (inspector) layout, not the dashboard "widget" layout: keeps
		     every parameter editor compact and consistent inside a node socket. -->
		<EditorComponent node={parameter} {presentation} />
	</div>
{/if}

<style>
	.socket-default-editor {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		inline-size: 100%;
		min-inline-size: 0;
		block-size: 1.2rem;
		font-size: 0.62rem;
	}

	/* One consistent, compact control height/size for every editor type. */
	.socket-default-editor :global(input),
	.socket-default-editor :global(select),
	.socket-default-editor :global(button),
	.socket-default-editor :global(textarea) {
		min-block-size: 1.05rem;
		font-size: 0.62rem;
	}

	.socket-default-editor :global(.number-property-container),
	.socket-default-editor :global(.editor-checkbox-shell),
	.socket-default-editor :global(.multi-number-editor),
	.socket-default-editor :global(.color-picker-editor) {
		inline-size: 100%;
		min-inline-size: 0;
		block-size: 1.2rem;
	}

	.socket-default-editor :global(.slider-wrapper) {
		display: none;
	}

	.socket-default-editor :global(.number-property-container) {
		justify-content: stretch;
		gap: 0;
	}

	.socket-default-editor :global(.number-field) {
		inline-size: 100%;
		width: 100%;
		max-inline-size: 100%;
		margin-inline-start: 0;
		padding-inline: 0.25rem;
	}

	.socket-default-editor :global(.single-number-editor) {
		gap: 0.15rem;
	}

	/* Text fields have plenty of room in the inspector; keep them modest in a node. */
	.socket-default-editor :global(.string-editor) {
		inline-size: 100%;
		max-inline-size: 8rem;
		font-size: 0.62rem;
	}

	/* Trigger: small, right-aligned, intrinsic width (not stretched). */
	.socket-default-editor :global(.trigger) {
		flex: 0 0 auto;
		inline-size: 2.6rem;
		block-size: 1.1rem;
	}

	/* Boolean: a small checkbox, not a full-width tile. */
	.socket-default-editor :global(.editor-checkbox) {
		inline-size: 0.95rem;
		block-size: 0.95rem;
		margin: 0;
	}

	.socket-default-editor :global(.color-picker-editor) {
		max-inline-size: 100%;
	}
</style>
