import { test } from 'node:test';
import assert from 'node:assert/strict';
import { matrixFeedback } from '../render-feedback.ts';
import type { MatrixViewportSnapshot } from '../../media/generated/MatrixViewportSnapshot';
import type { PreviewStatus } from '../../preview/generated/PreviewStatus';
import type { MatrixPlanStamp } from '../../preview/generated/MatrixPlanStamp';

const reference = { media: 'matrix', channel: 'final' };
// This selector reads identity only; other viewport fields belong to the authoring fixtures.
const snapshot = {
	project: 'project-instance',
	media: 'matrix',
	final_output: { reference }
} as MatrixViewportSnapshot;
const stamp = (column: string, launch: string): MatrixPlanStamp => ({
	revision: 1,
	columns: [column],
	transitioning: false,
	playback: {
		desired_column: column,
		launch_id: launch,
		transition: { duration_ms: 0, curve: 'Linear' }
	}
});
const status = (): PreviewStatus => ({
	source: { kind: 'Channel', reference },
	extent: null,
	error: null,
	message: null,
	matrix: {
		project: snapshot.project,
		stage: 'Published',
		availability: 'Ready',
		desired: stamp('new', 'new-launch'),
		installed: null,
		submitted: null,
		error: null
	}
});

test('native Matrix feedback requires the current project instance, final channel and connection', () => {
	const current = status();
	assert.equal(matrixFeedback(snapshot, current, true), current.matrix);
	assert.equal(matrixFeedback(snapshot, current, false), null);
	assert.equal(matrixFeedback(snapshot, undefined, true), null);
	assert.equal(matrixFeedback(null, current, true), null);
	assert.equal(matrixFeedback({ ...snapshot, project: 'reloaded-same-ids' }, current, true), null);
	assert.equal(
		matrixFeedback(snapshot, { ...current, source: { kind: 'Screen', screen: 'final' } }, true),
		null
	);
	assert.equal(
		matrixFeedback(
			snapshot,
			{ ...current, source: { kind: 'Channel', reference: { ...reference, channel: 'other' } } },
			true
		),
		null
	);
	assert.equal(
		matrixFeedback(
			snapshot,
			{ ...current, source: { kind: 'Channel', reference: { channel: 'final', media: 'matrix' } } },
			true
		),
		current.matrix
	);
});

test('preparation cannot invent submission; a newer launch leaves its prior observed output separate', () => {
	const current = status();
	assert.equal(matrixFeedback(snapshot, current, true)?.submitted, null);
	current.matrix!.stage = 'Preparing';
	current.matrix!.installed = stamp('old', 'old-launch');
	current.matrix!.submitted = stamp('old', 'old-launch');
	const pending = matrixFeedback(snapshot, current, true)!;
	assert.equal(pending.desired.playback.launch_id, 'new-launch');
	assert.equal(pending.installed?.playback.launch_id, 'old-launch');
	assert.equal(pending.submitted?.playback.desired_column, 'old');
});
