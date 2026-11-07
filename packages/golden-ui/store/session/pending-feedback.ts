import type { WorkbenchUserActionPhase } from './action-activity.svelte';

export const PENDING_FEEDBACK_DELAY_MS = 300;
export const PENDING_FEEDBACK_MIN_VISIBLE_MS = 350;
export const PENDING_FEEDBACK_GRACE_MS = 150;

export interface PendingFeedbackInput {
	owner: object;
	enabled: boolean;
	count: number;
	oldestQueuedAtMs: number | null;
	label: string | null;
	phase: WorkbenchUserActionPhase | null;
}
export interface PendingFeedbackView {
	visible: boolean;
	label: string;
	phase: string;
	phaseLabel: string;
	count: number;
}

function phaseLabel(phase: WorkbenchUserActionPhase | null) {
	switch (phase) {
		case 'queued':
			return 'Queued';
		case 'received':
			return 'Received';
		case 'applied':
			return 'Finishing';
		case 'rejected':
			return 'Failed';
		default:
			return 'Processing';
	}
}

/** One-shot presentation deadlines; a stream of short operations never becomes slow work. */
export class PendingFeedback {
	private input: PendingFeedbackInput | null = null;
	private view: PendingFeedbackView = {
		visible: false,
		label: '',
		phase: '',
		phaseLabel: '',
		count: 0
	};
	private shownAt = 0;
	private quietAt: number | null = null;
	private timer: ReturnType<typeof setTimeout> | null = null;
	private timerAt: number | null = null;
	private disposed = false;
	constructor(
		private changed: (view: PendingFeedbackView) => void,
		private nowMs = () => (typeof performance !== 'undefined' ? performance.now() : Date.now())
	) {}
	update(input: PendingFeedbackInput) {
		if (this.disposed) return;
		if (this.input?.owner !== input.owner || !input.enabled) this.reset();
		this.input = input;
		if (input.enabled) this.refresh();
	}
	dispose() {
		this.disposed = true;
		this.arm(null);
		this.input = null;
	}
	private publish(next: PendingFeedbackView) {
		const old = this.view;
		if (
			old.visible === next.visible &&
			old.label === next.label &&
			old.phase === next.phase &&
			old.phaseLabel === next.phaseLabel &&
			old.count === next.count
		)
			return;
		this.view = next;
		this.changed(next);
	}
	private arm(deadline: number | null) {
		if (this.timerAt === deadline) return;
		if (this.timer !== null) clearTimeout(this.timer);
		this.timer = null;
		this.timerAt = deadline;
		if (deadline !== null)
			this.timer = setTimeout(
				() => {
					this.timer = null;
					this.timerAt = null;
					this.refresh();
				},
				Math.max(0, deadline - this.nowMs())
			);
	}
	private reset() {
		this.arm(null);
		this.quietAt = null;
		this.publish({ ...this.view, visible: false });
	}
	private refresh() {
		const input = this.input;
		if (this.disposed || !input?.enabled) return;
		const now = this.nowMs();
		const pending = input.count > 0 && input.oldestQueuedAtMs !== null;
		const eligibleAt = pending ? input.oldestQueuedAtMs! + PENDING_FEEDBACK_DELAY_MS : null;
		if (pending && (input.phase === 'rejected' || now >= eligibleAt!)) {
			if (!this.view.visible) this.shownAt = now;
			this.quietAt = null;
			this.publish({
				visible: true,
				label: input.label ?? 'Applying changes',
				phase: input.phase ?? 'accepted',
				phaseLabel: phaseLabel(input.phase),
				count: input.count
			});
			this.arm(null);
			return;
		}
		let nextDeadline = eligibleAt;
		if (this.view.visible) {
			this.quietAt ??= now;
			// An overlapping successor may have only just started. Give it one full
			// eligibility interval, without letting each newer fast edit extend that grace.
			const grace = pending ? PENDING_FEEDBACK_DELAY_MS : PENDING_FEEDBACK_GRACE_MS;
			const hideAt = Math.max(this.shownAt + PENDING_FEEDBACK_MIN_VISIBLE_MS, this.quietAt + grace);
			if (now >= hideAt) {
				this.quietAt = null;
				this.publish({ ...this.view, visible: false });
			} else {
				if (!pending && this.view.phase !== 'rejected')
					this.publish({
						...this.view,
						label: 'Changes applied',
						phase: 'applied',
						phaseLabel: 'Done',
						count: 0
					});
				else this.publish({ ...this.view, count: input.count });
				nextDeadline = nextDeadline === null ? hideAt : Math.min(hideAt, nextDeadline);
			}
		}
		this.arm(nextDeadline);
	}
}
