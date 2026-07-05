import type { ElementType, HTMLAttributes } from "react";

import "./Card.css";

interface Props extends HTMLAttributes<HTMLDivElement> {
  as?: "div" | "article" | "section";
  padding?: "sm" | "md" | "lg";
  tone?: "default" | "alt" | "danger-outline";
}

/**
 * Superfície sólida elevada — base de painéis e cards (login, emuladores).
 * Nunca usa transparência; a distinção entre camadas vem de cor sólida e sombra.
 */
export function Card({ as = "div", padding = "md", tone = "default", className, ...rest }: Props) {
  const Tag = as as ElementType;
  const classes = ["rs-card", `rs-card-pad-${padding}`, `rs-card-${tone}`, className ?? ""]
    .filter(Boolean)
    .join(" ");

  return <Tag className={classes} {...rest} />;
}
