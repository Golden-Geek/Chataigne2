import type { UiDataPlane } from '../generated/rust_protocol/UiDataPlane';
import type { EventTime, UiSubscriptionScope } from '../types';
import type { StagedFrameBatchScheduler } from './staged-frame-batches';

export interface SubscriptionResyncState {
	scope: UiSubscriptionScope;
	cursor?: EventTime;
	closed: boolean;
	stagedFrames: StagedFrameBatchScheduler;
	subscribedSocketGeneration: number | null;
	resyncing: boolean;
	resyncGeneration: number;
	resyncPlane: UiDataPlane | undefined;
	resyncReason: string;
	resyncRetryTimer: ReturnType<typeof setTimeout> | null;
}

interface SubscriptionResyncCoordinatorOptions<TState extends SubscriptionResyncState> {
	retryMs: number;
	getSocketGeneration(): number;
	isSocketGenerationOpen(generation: number): boolean;
	requestSnapshot?: (
		scope: UiSubscriptionScope,
		plane: UiDataPlane | undefined,
		reason: string
	) => EventTime | Promise<EventTime>;
	sendSubscribe(subscriptionId: string, state: TState, generation: number): void;
	sendUnsubscribe(subscriptionId: string, generation: number): void;
}

export interface SubscriptionResyncCoordinator<TState extends SubscriptionResyncState> {
	begin(
		subscriptionId: string,
		state: TState,
		plane: UiDataPlane | undefined,
		reason: string
	): void;
	resumeAfterSocketOpen(subscriptionId: string, state: TState, generation: number): boolean;
	socketDisconnected(state: TState): void;
	dispose(state: TState): void;
}

export const createSubscriptionResyncCoordinator = <
	TState extends SubscriptionResyncState
>(
	options: SubscriptionResyncCoordinatorOptions<TState>
): SubscriptionResyncCoordinator<TState> => {
	const clearRetry = (state: TState): void => {
		if (state.resyncRetryTimer !== null) {
			clearTimeout(state.resyncRetryTimer);
			state.resyncRetryTimer = null;
		}
	};

	const startAttempt = (
		subscriptionId: string,
		state: TState,
		socketGeneration: number
	): void => {
		if (
			state.closed ||
			!state.resyncing ||
			!options.isSocketGenerationOpen(socketGeneration)
		) {
			return;
		}
		const requestSnapshot = options.requestSnapshot;
		if (!requestSnapshot) {
			console.error(
				`[ui ws] subscription ${subscriptionId} cannot resync without a snapshot callback`
			);
			return;
		}

		clearRetry(state);
		state.resyncGeneration += 1;
		const attemptGeneration = state.resyncGeneration;
		const plane = state.resyncPlane;
		const reason = state.resyncReason;
		void Promise.resolve()
			.then(() => requestSnapshot(state.scope, plane, reason))
			.then((boundary) => {
				if (
					state.closed ||
					!state.resyncing ||
					state.resyncGeneration !== attemptGeneration ||
					!options.isSocketGenerationOpen(socketGeneration)
				) {
					return;
				}
				state.stagedFrames.reset();
				state.cursor = boundary;
				state.resyncing = false;
				state.resyncPlane = undefined;
				state.resyncReason = '';
				options.sendSubscribe(subscriptionId, state, socketGeneration);
			})
			.catch((error: unknown) => {
				if (
					state.closed ||
					!state.resyncing ||
					state.resyncGeneration !== attemptGeneration ||
					!options.isSocketGenerationOpen(socketGeneration)
				) {
					return;
				}
				const message = error instanceof Error ? error.message : 'unknown snapshot error';
				console.error(`[ui ws] subscription ${subscriptionId} resync failed: ${message}`);
				clearRetry(state);
				state.resyncRetryTimer = setTimeout(() => {
					state.resyncRetryTimer = null;
					startAttempt(subscriptionId, state, socketGeneration);
				}, options.retryMs);
			});
	};

	const begin = (
		subscriptionId: string,
		state: TState,
		plane: UiDataPlane | undefined,
		reason: string
	): void => {
		state.resyncPlane = plane;
		state.resyncReason = reason;
		if (state.resyncing) {
			return;
		}

		state.resyncing = true;
		state.resyncGeneration += 1;
		state.stagedFrames.reset();
		clearRetry(state);
		const subscribedGeneration = state.subscribedSocketGeneration;
		state.subscribedSocketGeneration = null;
		if (subscribedGeneration !== null) {
			options.sendUnsubscribe(subscriptionId, subscribedGeneration);
		}
		startAttempt(subscriptionId, state, options.getSocketGeneration());
	};

	return {
		begin,
		resumeAfterSocketOpen: (subscriptionId, state, generation) => {
			if (!state.resyncing) {
				return false;
			}
			startAttempt(subscriptionId, state, generation);
			return true;
		},
		socketDisconnected: (state) => {
			state.stagedFrames.reset();
			state.subscribedSocketGeneration = null;
			clearRetry(state);
			if (state.resyncing) {
				state.resyncGeneration += 1;
			}
		},
		dispose: (state) => {
			state.resyncGeneration += 1;
			clearRetry(state);
		}
	};
};
