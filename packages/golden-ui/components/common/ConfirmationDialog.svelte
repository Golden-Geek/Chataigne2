<script lang="ts">
	import { tick } from 'svelte';

	export interface ConfirmationDialogAction {
		id: string;
		label: string;
		shortcut?: string;
		tone?: 'primary' | 'danger' | 'neutral';
		defaultFocus?: boolean;
		disabled?: boolean;
		action: () => void | Promise<void>;
	}

	let {
		open,
		title,
		message,
		errorMessage = null,
		actions,
		titleId = 'gc-confirmation-title'
	}: {
		open: boolean;
		title: string;
		message: string;
		errorMessage?: string | null;
		actions: ConfirmationDialogAction[];
		titleId?: string;
	} = $props();

	let dialogElement: HTMLDivElement | null = $state(null);

	const runAction = (action: ConfirmationDialogAction): void => {
		if (action.disabled) return;
		void action.action();
	};

	const shortcutMatches = (action: ConfirmationDialogAction, key: string): boolean => {
		return (
			action.shortcut
				?.split('/')
				.map((shortcut) => shortcut.trim().toLowerCase())
				.includes(key) ?? false
		);
	};

	const handleKeydown = (event: KeyboardEvent): void => {
		if (!open || event.ctrlKey || event.metaKey || event.altKey) return;
		const normalizedKey = event.key.trim().toLowerCase();
		const action =
			actions.find((candidate) => shortcutMatches(candidate, normalizedKey)) ??
			(normalizedKey === 'enter'
				? actions.find((candidate) => candidate.defaultFocus)
				: normalizedKey === 'escape'
					? actions[actions.length - 1]
					: undefined);
		if (!action || action.disabled) return;
		event.preventDefault();
		event.stopPropagation();
		runAction(action);
	};

	$effect(() => {
		if (!open || typeof window === 'undefined') return;
		window.addEventListener('keydown', handleKeydown, true);
		return () => {
			window.removeEventListener('keydown', handleKeydown, true);
		};
	});

	$effect(() => {
		if (!open) return;
		void tick().then(() => {
			const target =
				dialogElement?.querySelector<HTMLButtonElement>('[data-default-focus="true"]') ??
				dialogElement?.querySelector<HTMLButtonElement>('button:not(:disabled)');
			target?.focus();
		});
	});
</script>

{#if open}
	<div class="gc-confirmation-overlay">
		<div
			bind:this={dialogElement}
			class="gc-confirmation-dialog"
			role="dialog"
			aria-modal="true"
			aria-labelledby={titleId}>
			<div class="gc-confirmation-copy">
				<h2 id={titleId}>{title}</h2>
				<p>{message}</p>
				{#if errorMessage}
					<p class="gc-confirmation-error" role="alert" aria-live="assertive">
						{errorMessage}
					</p>
				{/if}
			</div>
			<div class="gc-confirmation-actions">
				{#each actions as action (action.id)}
					<button
						type="button"
						class:is-primary={action.tone === 'primary'}
						class:is-danger={action.tone === 'danger'}
						disabled={action.disabled}
						data-default-focus={action.defaultFocus ? 'true' : undefined}
						onclick={() => runAction(action)}>
						<span>{action.label}</span>
						{#if action.shortcut}
							<span class="shortcut">{action.shortcut}</span>
						{/if}
					</button>
				{/each}
			</div>
		</div>
	</div>
{/if}

<style>
	.gc-confirmation-overlay {
		position: fixed;
		inset: 0;
		z-index: 1400;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 1.25rem;
		background: linear-gradient(180deg, rgb(9 14 21 / 52%), rgb(9 14 21 / 76%));
		backdrop-filter: blur(0.45rem);
	}

	.gc-confirmation-dialog {
		inline-size: min(30rem, calc(100vw - 2.5rem));
		display: grid;
		gap: 1.25rem;
		padding: 1.3rem;
		border-radius: 1rem;
		background: linear-gradient(180deg, rgb(24 30 39 / 96%), rgb(14 19 27 / 98%));
		border: 0.06rem solid rgb(from var(--gc-color-text) r g b / 16%);
		box-shadow:
			0 1.4rem 3rem rgb(0 0 0 / 34%),
			0 0 0 0.06rem rgb(from var(--gc-color-focus) r g b / 10%);
	}

	.gc-confirmation-copy {
		display: grid;
		gap: 0.55rem;
	}

	.gc-confirmation-copy h2 {
		margin: 0;
		font-size: 1rem;
		font-weight: 700;
		letter-spacing: 0;
		color: rgb(from var(--gc-color-text) r g b / 96%);
	}

	.gc-confirmation-copy p {
		margin: 0;
		font-size: 0.86rem;
		line-height: 1.5;
		color: rgb(from var(--gc-color-text) r g b / 72%);
	}

	.gc-confirmation-error {
		padding: 0.7rem 0.8rem;
		border-radius: 0.7rem;
		border: 0.06rem solid rgb(214 120 96 / 38%);
		background: rgb(125 49 35 / 18%);
		color: rgb(255 196 178);
	}

	.gc-confirmation-actions {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(7.5rem, 1fr));
		gap: 0.7rem;
	}

	.gc-confirmation-actions button {
		display: grid;
		gap: 0.2rem;
		align-content: center;
		justify-items: start;
		min-block-size: 4rem;
		padding: 0.8rem 0.95rem;
		border-radius: 0.85rem;
		border: 0.06rem solid rgb(from var(--gc-color-text) r g b / 14%);
		background: rgb(from var(--gc-color-text) r g b / 7%);
		color: rgb(from var(--gc-color-text) r g b / 92%);
		text-align: left;
		transition:
			transform 0.16s ease,
			border-color 0.16s ease,
			background-color 0.16s ease,
			opacity 0.16s ease;
	}

	.gc-confirmation-actions button:hover:not(:disabled),
	.gc-confirmation-actions button:focus-visible:not(:disabled) {
		transform: translateY(-0.06rem);
		border-color: rgb(from var(--gc-color-focus) r g b / 42%);
		background: rgb(from var(--gc-color-text) r g b / 11%);
	}

	.gc-confirmation-actions button.is-primary {
		background: linear-gradient(180deg, rgb(78 133 214 / 58%), rgb(54 97 164 / 72%));
		border-color: rgb(121 175 255 / 44%);
	}

	.gc-confirmation-actions button.is-danger {
		background: linear-gradient(180deg, rgb(134 76 62 / 42%), rgb(109 57 45 / 56%));
		border-color: rgb(214 120 96 / 28%);
	}

	.gc-confirmation-actions button:disabled {
		opacity: 0.62;
		transform: none;
		cursor: default;
	}

	.shortcut {
		font-size: 0.68rem;
		letter-spacing: 0.04em;
		color: rgb(from var(--gc-color-text) r g b / 62%);
	}
</style>
