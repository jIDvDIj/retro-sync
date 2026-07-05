import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./i18n";
import "./styles/tokens.css";

// Tema padrão; o toggle claro/escuro troca este atributo na raiz.
document.documentElement.dataset.theme ??= "dark";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
