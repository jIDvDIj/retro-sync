import type { ElementType, HTMLAttributes } from "react";

import "./Badge.css";

interface Props extends HTMLAttributes<HTMLSpanElement> {
  tone?: "success" | "warning" | "danger" | "info" | "neutral" | "brand";
  as?: "span" | "button";
}

/**
 * Etiqueta/status compacto (badge/chip). Cobre status de emulador, categoria
 * de jogo e a tag de dispositivo — sempre com cor sólida (tint), nunca rgba
 * translúcido em fundo.
 */
export function Badge({ tone = "neutral", as = "span", className, ...rest }: Props) {
  const Tag = as as ElementType;
  const classes = ["rs-badge", `rs-badge-${tone}`, className ?? ""].filter(Boolean).join(" ");

  return <Tag className={classes} {...rest} />;
}
