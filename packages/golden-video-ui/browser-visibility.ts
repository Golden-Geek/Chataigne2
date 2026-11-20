/** Register image demand only while the document and its clipped image aperture are visible. */
export function observeBrowserPreview(element: HTMLElement, changed: (visible: boolean) => void) {
	const document = element.ownerDocument;
	let intersecting = false;
	const report = () => changed(intersecting && !document.hidden);
	const observer = new IntersectionObserver(([entry]) => {
		intersecting = !!entry?.isIntersecting && entry.intersectionRatio > 0;
		report();
	});
	observer.observe(element);
	document.addEventListener('visibilitychange', report);
	return () => {
		observer.disconnect();
		document.removeEventListener('visibilitychange', report);
	};
}
