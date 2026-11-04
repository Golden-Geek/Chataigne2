import type { DockviewIDisposable, ITabRenderer, TabPartInitParameters } from 'dockview-core';

export const createGoldenTabRenderer = (
	iconForType: (type: string) => string | undefined = () => undefined
): ITabRenderer => {
	const element = document.createElement('div');
	element.className = 'gc-dv-tab';

	const label = document.createElement('span');
	label.className = 'gc-dv-tab-label';

	const closeButton = document.createElement('button');
	closeButton.type = 'button';
	closeButton.className = 'gc-dv-tab-close';
	closeButton.setAttribute('aria-label', 'Close panel');
	closeButton.setAttribute('title', 'Close');

	element.append(label, closeButton);

	let disposables: DockviewIDisposable[] = [];

	return {
		element,
		init: (parameters: TabPartInitParameters): void => {
			label.textContent = parameters.title;
			element.dataset.panelType = parameters.api.component;
			const source = iconForType(parameters.api.component);
			if (source) {
				const icon = document.createElement('img');
				icon.className = 'gc-dv-tab-icon';
				icon.src = source;
				icon.alt = '';
				icon.draggable = false;
				Object.assign(icon.style, {
					width: '1em',
					height: '1em',
					objectFit: 'contain',
					flex: '0 0 auto'
				});
				element.insertBefore(icon, label);
			}

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

			closeButton.addEventListener('click', onClick);
			closeButton.addEventListener('mousedown', stopMouseDown);

			disposables = [
				onTitleChange,
				{
					dispose: () => {
						closeButton.removeEventListener('click', onClick);
						closeButton.removeEventListener('mousedown', stopMouseDown);
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
