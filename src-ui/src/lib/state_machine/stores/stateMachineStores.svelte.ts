import { AlchemistGraphStore } from './alchemistGraphStore.svelte';
import { AlchemistLibraryStore } from './alchemistLibraryStore.svelte';
import { AlchemistTypeStore } from './alchemistTypeStore.svelte';
import { ProcessorStore } from './processorStore.svelte';
import { RuntimeDebugStore } from './runtimeDebugStore.svelte';
import { StatechartStore } from './statechartStore.svelte';
import type { StateMachineProtocolBundle } from '../generated';

export class StateMachineStores {
	readonly statechart = new StatechartStore();
	readonly processors = new ProcessorStore();
	readonly graph = new AlchemistGraphStore();
	readonly types = new AlchemistTypeStore();
	readonly library = new AlchemistLibraryStore();
	readonly debug = new RuntimeDebugStore();

	replace(bundle: StateMachineProtocolBundle): void {
		for (const delta of bundle.statechart_deltas) {
			this.statechart.applyDelta(delta);
		}
		this.processors.replace(bundle.processors, bundle.diagnostics);
		this.graph.replace(bundle.graph_nodes, bundle.graph_edges);
		this.types.replace(bundle.socket_compatibility);
		this.debug.clear();
		for (const delta of bundle.runtime_debug) {
			this.debug.apply(delta);
		}
	}
}

export const stateMachineStores = new StateMachineStores();
