import { describe, expect, it } from 'vitest';
import { fromRustAck, toRustIntent } from '../codecs/documents';
import type { UiAck } from '../../generated/rust_protocol/UiAck';
import type { UiEditIntent } from '../../types';

describe('app operation request/reply transport', () => {
	it('preserves the exact creation receipt through the ordinary transport decoder', () => {
		const wire: UiAck = {
			success: true,
			status: 'applied',
			created_node: { node_id: 42, uuid: 'committed-root' },
			history: {
				can_undo: true,
				can_redo: false,
				undo_len: 1,
				redo_len: 0,
				active_edit_session: false,
				current_history_state_id: 1
			}
		};
		expect(fromRustAck(wire).created_node).toEqual(wire.created_node);
		expect(fromRustAck(wire).result).toBeUndefined();
	});
	it('preserves app-owned request DTOs without interpreting domain fields', () => {
		const intent: UiEditIntent = {
			kind: 'invokeAppOperation',
			namespace: 'fixture',
			operation: 'paste',
			payload: { fragment: { source: 42 }, position: [3, 4] }
		};
		expect(toRustIntent(intent)).toEqual(intent);
	});
	it.each([undefined, null, { source: 42, created: 77, uuid: 'exact identity' }])(
		'preserves authoritative replies, including explicit null (%j)',
		(result) => {
			const ack: UiAck = {
				success: true,
				status: 'applied',
				result: result === undefined ? undefined : { value: result },
				history: {
					can_undo: false,
					can_redo: false,
					undo_len: 0,
					redo_len: 0,
					active_edit_session: false,
					current_history_state_id: 0
				}
			};
			expect(fromRustAck(ack).result?.value).toEqual(result);
		}
	);
});
