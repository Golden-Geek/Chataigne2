import type { MatrixViewportSnapshot } from '../media/generated/MatrixViewportSnapshot';
import type { PreviewStatus } from '../preview/generated/PreviewStatus';

/** Persisted channel IDs survive reload; the ephemeral project instance fences native events. */
export function matrixFeedback(
	snapshot: MatrixViewportSnapshot | null,
	status: PreviewStatus | undefined,
	connected: boolean
) {
	if (
		!connected ||
		!snapshot ||
		status?.source.kind !== 'Channel' ||
		status.source.reference.media !== snapshot.media ||
		status.source.reference.channel !== snapshot.final_output.reference.channel ||
		status.matrix?.project !== snapshot.project
	)
		return null;
	return status.matrix;
}
