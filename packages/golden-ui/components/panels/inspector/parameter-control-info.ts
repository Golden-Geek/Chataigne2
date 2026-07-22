import type { UiParameterControlMode } from "../../../types";

export type ParameterControlInfoRefresh = {
  mode: UiParameterControlMode;
  nodeChanged: boolean;
  enteredContextLink: boolean;
  openedMenu: boolean;
  finishedLoading: boolean;
};

export const shouldFetchParameterControlInfo = ({
  mode,
  nodeChanged,
  enteredContextLink,
  openedMenu,
  finishedLoading,
}: ParameterControlInfoRefresh): boolean =>
  openedMenu ||
  enteredContextLink ||
  (mode === "contextLink" && (nodeChanged || finishedLoading));
