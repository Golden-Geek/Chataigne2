import { onDestroy, untrack } from 'svelte';
import { appState } from './app-state.svelte';
import { usePanelVisibility } from '../dockview/panel-visibility.svelte';
import { EditorParameterObserver } from './editor-parameter-observer';

/** Metadata routing remains live in retained tabs; value observation is scoped and visible.
 * New editors supply only their query, bounded backend owners and domain refresh operation. */
export function observeEditor<Q>(options: {
	query(): Q;
	refresh(query: Q, resolve: boolean): void;
	topics: readonly string[];
	owners?(): readonly string[];
	/** Limit metadata views to immediate Parameter ownership instead of nested editor objects. */
	parameterDepth?: number;
	interacting?(): boolean;
	pending?(): boolean;
	observe?(shown: boolean, query: Q): void;
}) {
	const panel = usePanelVisibility();
	const session = $derived(appState.session);
	const query = $derived(options.query());
	// Semantic query identity shields routing from unrelated graph projections.
	const key = $derived(JSON.stringify(query));
	let shownBefore = false;
	$effect(() => {
		void key;
		const current = session,
			shown = panel.visible;
		options.pending?.();
		options.interacting?.();
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.projectGeneration;
			void current.editSessionEpoch;
			if (!current.hasActiveEditSession) void current.currentHistoryStateId;
			for (const topic of options.topics) void current.getCustomEventSequence(topic);
		}
		untrack(() => {
			options.observe?.(shown, query);
			options.refresh(query, shown && !shownBefore);
			shownBefore = shown;
		});
	});
	const parameters = new EditorParameterObserver();
	let context: readonly unknown[] | null = null;
	$effect(() => {
		const current = session;
		if (!panel.visible || !current || current.status !== 'connected' || current.loading.visible) {
			parameters.reset();
			context = null;
			return;
		}
		const next = [current, current.snapshotGeneration, current.projectGeneration];
		const owners = options.owners?.() ?? [];
		const graph = current.graph.state;
		const interacting = options.interacting?.() === true;
		untrack(() => {
			if (!context || context.some((value, index) => !Object.is(value, next[index])))
				parameters.reset();
			context = next;
			if (parameters.observe(graph, owners, options.parameterDepth) && !interacting)
				options.refresh(query, false);
		});
	});
	onDestroy(() => parameters.reset());
}
