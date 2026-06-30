import type { Localized, Resources } from "../en/index";
import { auth } from "./auth";
import { common } from "./common";
import { errors } from "./errors";
import { settings } from "./settings";
import { sync } from "./sync";

/** Português (Brasil) — textos originais do app. */
export const pt: Localized<Resources> = {
  ...common,
  ...auth,
  ...sync,
  ...settings,
  ...errors,
};
