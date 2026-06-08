import type { RuntimeDebugDeltaDto } from '../generated';

export class RuntimeDebugStore {
	samplesByNode = $state(new Map<string, RuntimeDebugDeltaDto[]>());
	transitionTrace = $state<string[]>([]);
	commandTrace = $state<string[]>([]);
	maxSamplesPerNode = 64;

	apply(delta: RuntimeDebugDeltaDto): void {
		const samples = this.samplesByNode.get(delta.node_id) ?? [];
		samples.push(delta);
		if (samples.length > this.maxSamplesPerNode)
			samples.splice(0, samples.length - this.maxSamplesPerNode);
		this.samplesByNode.set(delta.node_id, samples);
	}

	clear(): void {
		this.samplesByNode.clear();
		this.transitionTrace = [];
		this.commandTrace = [];
	}
}
