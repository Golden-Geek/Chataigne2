<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { appState } from '../../../../store/workbench.svelte';
	import { EditorReadQueue } from '../../../../store/editor-read-queue';
	import ContextMenu from '../../../common/ContextMenu.svelte';
	import type { ContextMenuItem } from '../../../common/context-menu';
	import type { UiNodeDto } from '../../../../types';
	import type {
		ReferenceChoicePage,
		ReferenceChoiceProvider,
		ReferenceChoiceRequest
	} from '../reference-choice-provider';
	let {
		node,
		provider,
		readOnly = false,
		layoutMode = 'default',
		insideLabel = null
	}: {
		node: UiNodeDto;
		provider: ReferenceChoiceProvider;
		readOnly?: boolean;
		layoutMode?: 'default' | 'widget';
		insideLabel?: string | null;
	} = $props();
	const session = $derived(appState.session);
	const live = $derived(session?.getNodeData(node.node_id) ?? node);
	const value = $derived(
		live.data.kind === 'parameter' && live.data.param.value.kind === 'reference'
			? live.data.param.value
			: null
	);
	const blocked = $derived(
		readOnly ||
			!live.meta.enabled ||
			live.data.kind !== 'parameter' ||
			live.data.param.read_only ||
			session?.status !== 'connected' ||
			session.loading.visible ||
			session.hasActiveEditSession
	);
	let open = $state(false),
		busy = $state(false),
		error = $state('');
	let location = $state<string | null>(null);
	let cursor = $state<ReferenceChoiceRequest['cursor']>(null);
	let page = $state<ReferenceChoicePage | null>(null);
	let trigger = $state<HTMLButtonElement | null>(null);
	let alive = true;
	const reads = new EditorReadQueue();
	let identity: readonly unknown[] = [];
	$effect(() => {
		const current = session,
			selected = live,
			owner = provider;
		const navigation = { location, cursor, expanded: open };
		const context = [
			current,
			selected.uuid,
			current?.snapshotGeneration,
			current?.currentHistoryStateId
		];
		const revision = current && owner.revision?.(current);
		const selectedValue = value?.uuid;
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.currentHistoryStateId;
			void current.hasActiveEditSession;
			void current.editSessionEpoch;
		}
		untrack(() => {
			page = null;
			error = '';
			busy = false;
			if (
				!current ||
				current.status !== 'connected' ||
				current.loading.visible ||
				current.hasActiveEditSession
			) {
				reads.invalidate();
				open = false;
				return;
			}
			const reset = context.some((value, index) => !Object.is(value, identity[index]));
			if (reset) {
				identity = context;
				location = null;
				cursor = null;
				open = false;
			}
			const request = reset ? { location: null, cursor: null, expanded: false } : navigation;
			const key = JSON.stringify([selected.uuid, selectedValue, request, revision]);
			busy = true;
			reads.observe(
				current,
				key,
				() => alive && session === current && provider === owner && live.uuid === selected.uuid,
				() => owner.read(current, selected, request),
				(result) => {
					page = result;
				},
				(reason) => {
					error = String(reason);
				},
				() => {
					busy = false;
				}
			);
		});
	});
	onDestroy(() => {
		alive = false;
		reads.invalidate();
	});
	async function choose(key: string | null) {
		const current = session;
		if (!current || blocked || busy) return;
		busy = true;
		error = '';
		try {
			await provider.select(current, live, key);
			open = false;
		} catch (reason) {
			error = String(reason);
		} finally {
			busy = false;
		}
	}
	function navigate(next: string | null) {
		location = next;
		cursor = null;
	}
	const items = $derived.by((): ContextMenuItem[] => {
		if (busy) return [{ label: 'Loading…', disabled: true }];
		if (error) return [{ label: error, disabled: true }];
		if (!page) return [{ label: 'Choices unavailable', disabled: true }];
		const result: ContextMenuItem[] = [{ label: page.name, disabled: true }];
		if (page.choices.length === 0)
			result.push({ label: 'No choices on this page', disabled: true });
		if (page.location)
			result.push({
				label: '← Parent folder',
				action: () => navigate(page!.parent),
				closeOnSelect: false
			});
		if (page.previous)
			result.push({
				label: 'Previous page',
				action: () => {
					cursor = { key: page!.previous!, direction: 'previous' };
				},
				closeOnSelect: false
			});
		result.push(
			...page.choices.map((choice): ContextMenuItem => ({
				id: choice.key,
				label: choice.name,
				hint: choice.browse ? '›' : choice.key === value?.uuid ? '✓' : undefined,
				action: () => (choice.browse ? navigate(choice.key) : choose(choice.key)),
				closeOnSelect: false
			}))
		);
		if (page.next)
			result.push({
				label: 'Next page',
				action: () => {
					cursor = { key: page!.next!, direction: 'next' };
				},
				closeOnSelect: false
			});
		result.push(
			{ separator: true },
			{ label: page.emptyName, action: () => choose(null), closeOnSelect: false }
		);
		return result;
	});
	const label = $derived(
		page?.selection
			? `${page.selection.name}${page.selection.missing ? ' (unavailable)' : ''}`
			: value && value.uuid !== '00000000-0000-0000-0000-000000000000'
				? value.cached_name || value.uuid.slice(0, 8)
				: page?.emptyName || 'None'
	);
</script>

<div class="reference-choice" class:widget-layout={layoutMode === 'widget'}>
	<button
		type="button"
		class="choice-trigger"
		bind:this={trigger}
		disabled={blocked}
		aria-haspopup="menu"
		aria-expanded={open}
		title={label}
		onclick={() => {
			if (!blocked) open = !open;
		}}>
		{#if insideLabel}<span>{insideLabel}: </span>{/if}<span class="value">{label}</span><span
			aria-hidden="true">▾</span>
	</button>
	{#if error && !open}<span role="alert" class="error">{error}</span>{/if}
	<ContextMenu
		{open}
		{items}
		anchor={{ kind: 'element', element: trigger, placement: 'bottom-start' }}
		insideElements={[trigger]}
		closeOnSelect={false}
		preferPointerAnchor={false}
		onOpenChange={(next) => {
			open = next;
		}}
		minWidthRem={12}
		maxWidthCss="26rem" />
</div>

<style>
	.reference-choice {
		width: 100%;
		min-width: 0;
	}
	.choice-trigger {
		display: flex;
		align-items: center;
		gap: 0.4em;
		width: 100%;
		min-width: 0;
		padding: 0.25em 0.45em;
		text-align: left;
	}
	.value {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.error {
		color: var(--gc-color-error);
		font-size: 0.8em;
	}
</style>
