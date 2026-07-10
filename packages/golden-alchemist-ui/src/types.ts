import type { PortRef } from "@golden/graph-ui";

type Brand<Value, Name extends string> = Value & { readonly __brand: Name };

export type FormulaId = Brand<string, "FormulaId">;
export type SurfaceItemId = Brand<string, "SurfaceItemId">;

export interface FormulaCatalogItem {
  id: FormulaId;
  name: string;
  description: string;
  tags: readonly string[];
  builtIn: boolean;
}

export interface FormulaSurfaceInput {
  id: SurfaceItemId;
  label: string;
  target: PortRef;
  valueType: string;
  defaultValue: unknown;
}

export interface FormulaSurfaceOutput {
  id: SurfaceItemId;
  label: string;
  source: PortRef;
  valueType: string;
}

export interface FormulaSnapshot {
  formulaId: FormulaId;
  revision: number;
  catalog: readonly FormulaCatalogItem[];
  inputs: readonly FormulaSurfaceInput[];
  outputs: readonly FormulaSurfaceOutput[];
  runtimeOutputs?: Readonly<Record<string, unknown>>;
}

export type FormulaDeltaChange =
  | { kind: "catalog-upserted"; item: FormulaCatalogItem }
  | { kind: "catalog-removed"; formulaId: FormulaId }
  | { kind: "surface-input-upserted"; input: FormulaSurfaceInput }
  | { kind: "surface-input-removed"; inputId: SurfaceItemId }
  | { kind: "surface-output-upserted"; output: FormulaSurfaceOutput }
  | { kind: "surface-output-removed"; outputId: SurfaceItemId }
  | { kind: "runtime-output"; outputId: SurfaceItemId; value: unknown };

export interface FormulaDelta {
  before: number;
  after: number;
  changes: readonly FormulaDeltaChange[];
}
