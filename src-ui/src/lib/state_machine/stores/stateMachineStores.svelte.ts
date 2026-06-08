import { AlchemistGraphStore } from './alchemistGraphStore.svelte';
import { AlchemistLibraryStore } from './alchemistLibraryStore.svelte';
import { AlchemistTypeStore } from './alchemistTypeStore.svelte';
import { ProcessorStore } from './processorStore.svelte';
import { RuntimeDebugStore } from './runtimeDebugStore.svelte';
import { StatechartStore } from './statechartStore.svelte';

export class StateMachineStores {
	readonly statechart = new StatechartStore();
	readonly processors = new ProcessorStore();
	readonly graph = new AlchemistGraphStore();
	readonly types = new AlchemistTypeStore();
	readonly library = new AlchemistLibraryStore();
	readonly debug = new RuntimeDebugStore();
}
