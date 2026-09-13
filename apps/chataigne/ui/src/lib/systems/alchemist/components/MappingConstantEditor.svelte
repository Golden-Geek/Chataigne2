<script lang="ts">
	import type { MappingConstantDto } from '../../state_machine/generated';

	let {
		value,
		disabled = false,
		onCommit
	}: {
		value: MappingConstantDto | null;
		disabled?: boolean;
		onCommit: (value: MappingConstantDto) => void;
	} = $props();

	type ConstantKind = MappingConstantDto['kind'];
	const kinds: Array<{ id: ConstantKind; label: string }> = [
		{ id: 'float', label: 'Number' },
		{ id: 'int', label: 'Integer' },
		{ id: 'string', label: 'Text' },
		{ id: 'bool', label: 'Boolean' },
		{ id: 'vec2', label: 'Vec2' },
		{ id: 'vec3', label: 'Vec3' },
		{ id: 'color', label: 'Color' },
		{ id: 'duration', label: 'Duration' },
		{ id: 'unit', label: 'Unit' },
		{ id: 'raw', label: 'Golden value JSON' }
	];

	const valueText = (value: MappingConstantDto | null): string => {
		if (!value) return '';
		switch (value.kind) {
			case 'unit':
				return '';
			case 'color':
				return [value.red, value.green, value.blue, value.alpha].join(', ');
			case 'duration':
				return String(value.seconds);
			case 'raw':
				return value.json;
			case 'vec2':
			case 'vec3':
				return value.value.join(', ');
			case 'bool':
				return String(value.value);
			default:
				return String(value.value);
		}
	};

	let kind = $state<ConstantKind | ''>('');
	let text = $state('');
	let error = $state('');
	let observedSignature = '';
	$effect(() => {
		const signature = JSON.stringify(value);
		if (signature === observedSignature) return;
		observedSignature = signature;
		kind = value?.kind ?? '';
		text = valueText(value);
		error = '';
	});

	const parseNumbers = (input: string, count: number): number[] | null => {
		const parts = input.split(',').map((part) => part.trim());
		if (parts.some((part) => part === '')) return null;
		const numbers = parts.map(Number);
		return numbers.length === count && numbers.every((number) => Number.isFinite(number))
			? numbers
			: null;
	};

	const apply = (): void => {
		error = '';
		const numeric = Number(text);
		let next: MappingConstantDto;
		switch (kind) {
			case 'unit':
				next = { kind: 'unit' };
				break;
			case 'bool':
				if (text !== 'true' && text !== 'false') {
					error = 'Choose true or false.';
					return;
				}
				next = { kind: 'bool', value: text === 'true' };
				break;
			case 'int':
				if (!Number.isSafeInteger(numeric) || text.trim() === '') {
					error = 'Enter a whole number.';
					return;
				}
				next = { kind: 'int', value: numeric };
				break;
			case 'float':
				if (!Number.isFinite(numeric) || text.trim() === '') {
					error = 'Enter a finite number.';
					return;
				}
				next = { kind: 'float', value: numeric };
				break;
			case 'duration':
				if (!Number.isFinite(numeric) || numeric < 0 || text.trim() === '') {
					error = 'Enter a non-negative duration in seconds.';
					return;
				}
				next = { kind: 'duration', seconds: numeric };
				break;
			case 'string':
				next = { kind: 'string', value: text };
				break;
			case 'vec2': {
				const parts = parseNumbers(text, 2);
				if (!parts) {
					error = 'Enter two comma-separated numbers.';
					return;
				}
				next = { kind: 'vec2', value: [parts[0], parts[1]] };
				break;
			}
			case 'vec3': {
				const parts = parseNumbers(text, 3);
				if (!parts) {
					error = 'Enter three comma-separated numbers.';
					return;
				}
				next = { kind: 'vec3', value: [parts[0], parts[1], parts[2]] };
				break;
			}
			case 'color': {
				const parts = parseNumbers(text, 4);
				if (!parts) {
					error = 'Enter red, green, blue, and alpha.';
					return;
				}
				next = {
					kind: 'color',
					red: parts[0],
					green: parts[1],
					blue: parts[2],
					alpha: parts[3]
				};
				break;
			}
			case 'raw':
				try {
					JSON.parse(text);
				} catch {
					error = 'Enter valid Golden value JSON.';
					return;
				}
				next = { kind: 'raw', json: text };
				break;
			default:
				error = 'Choose a constant type.';
				return;
		}
		onCommit(next);
	};
</script>

<div class="mapping-constant-editor">
	<label>
		<span>Type</span>
		<select bind:value={kind} {disabled} aria-label="Constant type">
			<option value="">Choose type</option>
			{#each kinds as option (option.id)}
				<option value={option.id}>{option.label}</option>
			{/each}
		</select>
	</label>
	{#if kind === 'bool'}
		<label>
			<span>Value</span>
			<select bind:value={text} {disabled} aria-label="Constant value">
				<option value="">Choose value</option>
				<option value="true">True</option>
				<option value="false">False</option>
			</select>
		</label>
	{:else if kind !== 'unit' && kind !== ''}
		<label>
			<span>Value</span>
			<input
				type={kind === 'float' || kind === 'int' || kind === 'duration' ? 'number' : 'text'}
				bind:value={text}
				{disabled}
				aria-label="Constant value" />
		</label>
	{/if}
	<button type="button" onclick={apply} disabled={disabled || kind === ''}>Apply constant</button>
	{#if error}<small role="alert">{error}</small>{/if}
</div>

<style>
	.mapping-constant-editor {
		display: flex;
		flex-wrap: wrap;
		align-items: end;
		gap: 0.4rem;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
		min-inline-size: 5rem;
		flex: 1 1 7rem;
	}
	span,
	small {
		font-size: 0.72rem;
	}
	select,
	input,
	button {
		min-block-size: 1.8rem;
		padding: 0.2rem 0.35rem;
		border: 0.06rem solid var(--gc-color-border);
		border-radius: 0.3rem;
		background: var(--gc-color-background);
		color: var(--gc-color-text);
		font: inherit;
	}
	input,
	select {
		inline-size: 100%;
		min-inline-size: 0;
	}
	button {
		cursor: pointer;
	}
	small {
		flex-basis: 100%;
		color: var(--gc-color-danger, #e55);
	}
</style>
