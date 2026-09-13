/** Applies an authoritative insertion against the exact sibling count it was based on. */
export const insertChildrenIntoOrder = (
	previous: number[] | undefined,
	expectedBeforeCount: number,
	index: number,
	inserted: number[]
): number[] | undefined => {
	if (
		!previous ||
		previous.length !== expectedBeforeCount ||
		index < 0 ||
		index > previous.length
	) {
		return undefined;
	}
	return previous.slice(0, index).concat(inserted, previous.slice(index));
};
