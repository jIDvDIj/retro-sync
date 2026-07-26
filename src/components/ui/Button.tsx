import type { ButtonHTMLAttributes } from "react";

import "./Button.css";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost" | "danger";
  size?: "sm" | "md";
  fullWidth?: boolean;
}

/**
 * Botão base do app. Toda ação clicável fora dos modais (que mantêm o botão
 * cru como rede de segurança) deve usar este primitivo em vez de `<button>`.
 */
export function Button({
  variant = "primary",
  size = "md",
  fullWidth = false,
  className,
  ...rest
}: Props) {
  const classes = [
    "rs-button",
    `rs-button-${variant}`,
    `rs-button-${size}`,
    fullWidth ? "rs-button-full" : "",
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ");

  return <button className={classes} {...rest} />;
}
