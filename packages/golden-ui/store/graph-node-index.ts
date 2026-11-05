import type { NodeId } from '../types';
import { VersionedNodeMap } from './graph-index';

const hashIdentity = (uuid: string): NodeId => {
	let hash = 2166136261;
	for (let index = 0; index < uuid.length; index++) {
		hash = Math.imul(hash ^ uuid.charCodeAt(index), 16777619);
	}
	return hash >>> 0;
};

/** UUID lookup shares the graph's copy-on-write lifetime, including unpublished projections. */
export class IdentifiedNodeMap<Value extends { uuid: string }> extends VersionedNodeMap<Value> {
	private identities = new VersionedNodeMap<ReadonlyMap<string, NodeId>>();

	constructor(entries?: Iterable<readonly [NodeId, Value]>) {
		super();
		for (const [id, value] of entries ?? []) this.set(id, value);
	}

	override fork(): IdentifiedNodeMap<Value> {
		const fork = new IdentifiedNodeMap<Value>();
		this.forkInto(fork);
		fork.identities = this.identities.fork();
		return fork;
	}

	getByUuid(uuid: string): Value | undefined {
		const id = this.identities.get(hashIdentity(uuid))?.get(uuid);
		return id === undefined ? undefined : this.get(id);
	}

	override set(id: NodeId, value: Value): this {
		const previous = this.get(id);
		super.set(id, value);
		if (previous?.uuid !== value.uuid) {
			if (previous) this.removeIdentity(id, previous.uuid);
			const hash = hashIdentity(value.uuid);
			const bucket = new Map(this.identities.get(hash));
			bucket.set(value.uuid, id);
			this.identities.set(hash, bucket);
		}
		return this;
	}

	override delete(id: NodeId): boolean {
		const value = this.get(id);
		if (value) this.removeIdentity(id, value.uuid);
		return super.delete(id);
	}

	private removeIdentity(id: NodeId, uuid: string): void {
		const hash = hashIdentity(uuid);
		const previous = this.identities.get(hash);
		if (previous?.get(uuid) !== id) return;
		const bucket = new Map(previous);
		bucket.delete(uuid);
		if (bucket.size) this.identities.set(hash, bucket);
		else this.identities.delete(hash);
	}

	override clear(): void {
		super.clear();
		this.identities.clear();
	}
}
