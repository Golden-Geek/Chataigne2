import type { ChannelChoicePage } from '../media/generated/ChannelChoicePage';
import type { ChannelChoiceScope } from '../media/generated/ChannelChoiceScope';
import type { MediaEdit } from '../media/generated/MediaEdit';
import type { MediaEditResult } from '../media/generated/MediaEditResult';
import { ReadQueue } from './read-queue.ts';
export interface Connection {
	readonly status: string;
	readonly snapshotGeneration: number;
	readonly loading: { readonly visible: boolean };
	readonly currentHistoryStateId?: unknown;
	readonly hasActiveEditSession?: boolean;
}
export interface ChoiceState {
	page: ChannelChoicePage | null;
	search: string;
	start: number;
	loading: boolean;
	error: string;
}
export const choiceState = (): ChoiceState => ({
	page: null,
	search: '',
	start: 0,
	loading: false,
	error: ''
});
export class ChoiceReader<S extends Connection> {
	private queue = new ReadQueue();
	private session: () => S | null;
	readonly state: ChoiceState;
	private scope: ChannelChoiceScope;
	private invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>;
	private accepted: () => void;
	constructor(
		session: () => S | null,
		state: ChoiceState,
		scope: ChannelChoiceScope,
		invoke: (session: S, edit: MediaEdit) => Promise<MediaEditResult>,
		accepted: () => void = () => {}
	) {
		this.session = session;
		this.state = state;
		this.scope = scope;
		this.invoke = invoke;
		this.accepted = accepted;
	}
	reset() {
		this.queue.invalidate();
		this.state.page = null;
		this.state.loading = false;
		this.state.error = '';
	}
	refresh() {
		const session = this.session();
		if (!session || session.status !== 'connected' || session.loading.visible) {
			this.reset();
			return;
		}
		const generation = session.snapshotGeneration,
			history = session.currentHistoryStateId;
		const valid = () =>
			this.session() === session &&
			session.snapshotGeneration === generation &&
			session.currentHistoryStateId === history &&
			session.status === 'connected' &&
			!session.loading.visible;
		this.state.loading = true;
		const query = {
			scope: this.scope,
			search: this.state.search,
			start: this.state.start,
			count: 64
		};
		this.queue.request(
			() => this.invoke(session, { kind: 'InspectChoices', query }),
			(result) => {
				if (!valid()) return;
				if (result.kind !== 'Choices') {
					this.state.error = 'Unexpected channel choices';
					return;
				}
				this.state.page = result.page;
				this.state.error = '';
				this.accepted();
			},
			(error) => {
				if (valid()) this.state.error = String(error);
			},
			() => {
				this.state.loading = false;
			}
		);
	}
}
