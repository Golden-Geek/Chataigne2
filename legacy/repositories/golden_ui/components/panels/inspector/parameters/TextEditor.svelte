<!-- <script lang="ts">
	import CodeMirror from 'svelte-codemirror-editor';
	import { javascript } from '@codemirror/lang-javascript';
    import { oneDark } from "@codemirror/theme-one-dark";

	let {
		targets,
		expressionMode,
		expressionResultTag,
		property = $bindable(),
		definition,
		onStartEdit = null,
		onUpdate = null
	} = $props();
	let target = $derived(targets.length > 0 ? targets[0] : null);
</script> -->

<!-- <CodeMirror
	value={property.get()}
	class="text-editor-property {expressionMode} {expressionResultTag}"
	theme={oneDark}
	onchange={(value: string) => {
		// Apply filter function if defined
		let newValue = value;
		if (definition.filterFunction) {
			newValue = definition.filterFunction(newValue);
		}
		property.set(newValue);
		onUpdate && onUpdate();
	}}
	lang={javascript()}
/> -->

<!-- <textarea
	class="text-editor-property {expressionMode} {expressionResultTag}"
	disabled={definition.readOnly}
	onchange={(e) => {
		let newValue = (e.target as HTMLTextAreaElement).value;
		// Apply filter function if defined
		if (definition.filterFunction) {
			newValue = definition.filterFunction(newValue);
		}
		property.set(newValue);
		onUpdate && onUpdate();
	}}
	onfocus={() => onStartEdit && onStartEdit()}
	onblur={() => onUpdate && onUpdate()}>{property.get()}</textarea> -->

<!-- <style>
	:global {
		.text-editor-property {
			width: 100%;
			min-height: 10rem;
			background-color: var(--panel-bg-color);
		}
	}
</style> -->
