import type { WorkbenchSession } from 'golden_ui';
import {
	FORMULA_OPERATION_NAMESPACE,
	FORMULA_COPY_OPERATION,
	FORMULA_PASTE_OPERATION,
	FORMULA_CONNECT_OPERATION
} from './generated/authoring/constants';
import type { FormulaCopyRequest } from './generated/authoring/FormulaCopyRequest';
import type { FormulaFragment } from './generated/authoring/FormulaFragment';
import type { FormulaPasteRequest } from './generated/authoring/FormulaPasteRequest';
import type { FormulaPasteResult } from './generated/authoring/FormulaPasteResult';
import type { FormulaConnectRequest } from './generated/authoring/FormulaConnectRequest';
import type { FormulaConnectResult } from './generated/authoring/FormulaConnectResult';
import type { FormulaOwnerDto } from './generated/authoring/FormulaOwnerDto';

type OperationSession = Pick<WorkbenchSession, 'invokeAppOperation'>;
type OperationValue = Awaited<ReturnType<OperationSession['invokeAppOperation']>>;

const record = (value: OperationValue): value is { [key: string]: OperationValue } =>
	typeof value === 'object' && value !== null && !Array.isArray(value);

const nodeId = (value: OperationValue): value is number =>
	typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;

const ownerReply = (value: OperationValue): value is FormulaOwnerDto =>
	record(value) && nodeId(value.node) && typeof value.uuid === 'string';

/** Checks the reply envelope; authored document capsules remain opaque JSON. */
const fragmentReply = (value: OperationValue): value is FormulaFragment =>
	record(value) &&
	typeof value.version === 'number' &&
	typeof value.source_formula_uuid === 'string' &&
	Array.isArray(value.items) &&
	value.items.every(
		(item) =>
			record(item) &&
			'document' in item &&
			(item.position === null ||
				(Array.isArray(item.position) &&
					item.position.length === 2 &&
					item.position.every(
						(coordinate) => typeof coordinate === 'number' && Number.isFinite(coordinate)
					)))
	);

const pasteReply = (value: OperationValue): value is FormulaPasteResult =>
	record(value) &&
	Array.isArray(value.selected_nodes) &&
	value.selected_nodes.every(nodeId) &&
	Array.isArray(value.identities) &&
	value.identities.every(
		(identity) =>
			record(identity) &&
			typeof identity.source_uuid === 'string' &&
			typeof identity.uuid === 'string' &&
			nodeId(identity.node)
	);

export const copyFormulaFragment = async (
	session: OperationSession,
	request: FormulaCopyRequest
): Promise<FormulaFragment> => {
	const result = await session.invokeAppOperation({
		kind: 'invokeAppOperation',
		namespace: FORMULA_OPERATION_NAMESPACE,
		operation: FORMULA_COPY_OPERATION,
		payload: request
	});
	if (!fragmentReply(result)) throw new Error('Formula copy returned an invalid result.');
	return result;
};

export const pasteFormulaFragment = async (
	session: OperationSession,
	request: FormulaPasteRequest
): Promise<FormulaPasteResult> => {
	const result = await session.invokeAppOperation({
		kind: 'invokeAppOperation',
		namespace: FORMULA_OPERATION_NAMESPACE,
		operation: FORMULA_PASTE_OPERATION,
		payload: request
	});
	if (!pasteReply(result)) throw new Error('Formula paste returned an invalid result.');
	return result;
};

export const connectFormulaNodes = async (
	session: OperationSession,
	request: FormulaConnectRequest
): Promise<FormulaConnectResult> => {
	const result = await session.invokeAppOperation({
		kind: 'invokeAppOperation',
		namespace: FORMULA_OPERATION_NAMESPACE,
		operation: FORMULA_CONNECT_OPERATION,
		payload: request
	});
	if (
		!record(result) ||
		!ownerReply(result.connection) ||
		!Array.isArray(result.replaced) ||
		!result.replaced.every(ownerReply)
	) {
		throw new Error('Formula connect returned an invalid result.');
	}
	return { connection: result.connection, replaced: result.replaced };
};
