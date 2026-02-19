import type {
	DockviewIDisposable,
	ITabRenderer,
	TabPartInitParameters
} from "dockview-core";

export const createGoldenTabRenderer = (): ITabRenderer => {
	const element = document.createElement("div");
	element.className = "gc-dv-tab";

	const label = document.createElement("span");
	label.className = "gc-dv-tab-label";

	const closeButton = document.createElement("button");
	closeButton.type = "button";
	closeButton.className = "gc-dv-tab-close";
	closeButton.setAttribute("aria-label", "Close panel");
	closeButton.setAttribute("title", "Close");

	element.append(label, closeButton);

	let disposables: DockviewIDisposable[] = [];

	return {
		element,
		init: (parameters: TabPartInitParameters): void => {
			label.textContent = parameters.title;

			const onTitleChange = parameters.api.onDidTitleChange((event) => {
				label.textContent = event.title;
			});

			const onClick = (event: MouseEvent): void => {
				event.preventDefault();
				event.stopPropagation();
				parameters.api.close();
			};

			const stopMouseDown = (event: MouseEvent): void => {
				event.preventDefault();
				event.stopPropagation();
			};

			closeButton.addEventListener("click", onClick);
			closeButton.addEventListener("mousedown", stopMouseDown);

			disposables = [
				onTitleChange,
				{
					dispose: () => {
						closeButton.removeEventListener("click", onClick);
						closeButton.removeEventListener("mousedown", stopMouseDown);
					}
				}
			];
		},
		dispose: (): void => {
			for (const disposable of disposables) {
				disposable.dispose();
			}

			disposables = [];
		}
	};
};
