import type { SocketCompatibilityDto } from '../generated';

export class AlchemistTypeStore {
	compatibility = $state(new Map<string, SocketCompatibilityDto>());

	replace(entries: SocketCompatibilityDto[]): void {
		this.compatibility = new Map(
			entries.map((entry) => [this.key(entry.node_id, entry.socket_id), entry])
		);
	}

	forSocket(nodeId: string, socketId: string): SocketCompatibilityDto | null {
		return this.compatibility.get(this.key(nodeId, socketId)) ?? null;
	}

	private key(nodeId: string, socketId: string): string {
		return `${nodeId}:${socketId}`;
	}
}
