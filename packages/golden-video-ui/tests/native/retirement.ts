import { RetirementLayer } from '../../retirement-layer';

const layer = new RetirementLayer();
document.body.prepend(layer.element);
Object.assign(window, { __retirementLayer: layer });
