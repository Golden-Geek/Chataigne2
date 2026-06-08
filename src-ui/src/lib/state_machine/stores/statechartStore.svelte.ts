import type { StatechartDeltaDto, StateUiNodeDto } from '../generated';

export class StatechartStore {
	statesById = $state(new Map<string, StateUiNodeDto>());
	activeStateIds = $state(new Set<string>());
	selectedStateId = $state<string | null>(null);

	applyDelta(delta: StatechartDeltaDto): void {
		switch (delta.kind) {
			case 'upsert':
				this.statesById.set(delta.state.id, delta.state);
				this.setActive(delta.state.id, delta.state.active);
				break;
			case 'remove':
				this.statesById.delete(delta.state_id);
				this.activeStateIds.delete(delta.state_id);
				if (this.selectedStateId === delta.state_id) this.selectedStateId = null;
				break;
			case 'active_changed': {
				const state = this.statesById.get(delta.state_id);
				if (state) state.active = delta.active;
				this.setActive(delta.state_id, delta.active);
				break;
			}
		}
	}

	select(stateId: string | null): void {
		this.selectedStateId = stateId;
	}

	private setActive(stateId: string, active: boolean): void {
		if (active) this.activeStateIds.add(stateId);
		else this.activeStateIds.delete(stateId);
	}
}
