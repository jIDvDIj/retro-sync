import { auth } from "./auth";
import { common } from "./common";
import { errors } from "./errors";
import { settings } from "./settings";
import { sync } from "./sync";

/** Inglês — idioma padrão. A forma deste objeto é a fonte da verdade (`Resources`). */
export const en = { ...common, ...auth, ...sync, ...settings, ...errors } as const;

export type Resources = typeof en;

export type { Localized } from "../types";
