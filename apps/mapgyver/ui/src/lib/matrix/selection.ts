import type { MatrixSelection } from '../media/generated/MatrixSelection';

/** Protocol object member order is not part of selection identity. */
export function sameSelection(
	left: MatrixSelection | null | undefined,
	right: MatrixSelection | null | undefined
): boolean {
	if (left == null || right == null) return left == null && right == null;
	return (
		left.column === right.column &&
		(left.item == null || right.item == null
			? left.item == null && right.item == null
			: left.item.kind === right.item.kind && left.item.id === right.item.id)
	);
}
