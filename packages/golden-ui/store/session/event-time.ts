import type { EventTime } from '../../types';

export const compareEventTime = (left: EventTime, right: EventTime): number =>
	left.tick - right.tick || left.micro - right.micro || left.seq - right.seq;
