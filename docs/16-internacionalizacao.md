# 16 — Internacionalização (i18n)

> Torna a interface disponível em múltiplos idiomas. Idiomas iniciais: **English** (padrão)
> e **Português (Brasil)**. A infraestrutura aceita novos idiomas sem tocar nos componentes.

## O quê

Toda string visível ao usuário no frontend vem do i18n (`react-i18next`). O idioma:

- **padrão é inglês** (`en`) na primeira execução;
- é **escolhido pelo usuário** num seletor na seção "Language" do modal de configurações;
- é **persistido em `localStorage`** (`retrosync.language`) — não cruza a boundary, pois é
  concern puramente de apresentação.

O menu nativo da bandeja (Rust) também está em inglês (`Open` / `Sync now` / `Quit`).

## Por quê

O app nasceu todo em português. Para distribuí-lo publicamente, inglês como padrão alcança o
maior público; o português continua disponível num clique. Manter os textos centralizados em
arquivos de locale (em vez de espalhados no JSX) é o que permite adicionar um terceiro idioma
sem caçar strings componente a componente.

## Como

### Estrutura de arquivos (`src/i18n/`)

Os locales são divididos por **domínio semântico** — cada módulo cobre uma área funcional do
app. Adicionar um idioma = criar uma pasta nova com os mesmos cinco arquivos.

```
src/i18n/
  index.ts              ← init do i18next, SUPPORTED_LANGUAGES, helpers
  locales/
    types.ts            ← tipo utilitário Localized<T>
    en/                 ← idioma base (fonte da verdade)
      common.ts         ← common, app
      auth.ts           ← login, device, account
      sync.ts           ← sync, emulator, conflict
      settings.ts       ← settings, addEmulator
      errors.ts         ← errors
      index.ts          ← monta tudo + exporta Resources
    pt/                 ← Português (Brasil)
      common.ts / auth.ts / sync.ts / settings.ts / errors.ts
      index.ts          ← monta tudo tipado como Localized<Resources>
```

### Tipagem e paridade de chaves

`Localized<T>` (em `locales/types.ts`) converte todas as folhas de `T` para `string`.
Cada `pt/*.ts` importa o tipo do módulo `en/*.ts` correspondente e é tipado contra ele:

```ts
// pt/auth.ts
import type { auth as AuthEn } from "../en/auth";
export const auth: Localized<typeof AuthEn> = { ... };
```

Isso faz o **TypeScript exigir exatamente as mesmas chaves** — chave faltando ou sobrando
é erro de compilação, detectado antes mesmo do bundle.

### Validação no CI (`scripts/check-i18n.mjs`)

O CI roda `npm run i18n:check` (antes do build) que executa `tsc --noEmit`. Se qualquer
chave estiver faltando em qualquer locale, o passo falha com a mensagem exata do TypeScript
indicando o arquivo e a chave. Como o `tsc` já roda no build, este passo garante feedback
rápido e isolado em PRs que mexam apenas nos locales.

### `i18n/index.ts`

| Exportação | Uso |
| --- | --- |
| `SUPPORTED_LANGUAGES` | Alimenta o seletor de idioma no `SettingsModal` |
| `storedLanguage()` | Lê `localStorage` na inicialização |
| `changeLanguage(code)` | Persiste + troca o idioma ativo |
| `currentLocale()` | BCP-47 para `Intl`/`toLocaleString` (`en-US`, `pt-BR`) |

`CustomTypeOptions` (augmentação de `i18next`) torna `t("chave")` checado em tempo de
compilação — chave inexistente é erro de tipo. A inicialização é **síncrona** e os recursos
são **embutidos** (sem backend HTTP, sem Suspense): os componentes renderizam já no idioma
certo.

### Tradução de erros do backend

O `AppError` do Rust serializa com `code` (enum fechado) + `message` + `detail`. O frontend
localiza o prefixo pelo `code` (chaves `errors.<code>`) e anexa o `detail`:

```
errorMessage(io)  →  t("errors.io") + ": " + detail   // "I/O error: <detalhe da lib>"
```

`code: "other"` não tem prefixo a traduzir — cai no `detail`/`message` como veio. O acesso
é via hook `useErrorMessage()`, que devolve um tradutor com identidade estável (só muda quando
o idioma muda), preservando os `useCallback`/`useEffect` dos hooks de dados.

### Datas e plurais

`toLocaleString` usa `currentLocale()` em vez do fixo `pt-BR`. Plurais e interpolação ficam
no i18next: chaves `*_one`/`*_other` e `{{count}}`/`{{when}}`.

## Como adicionar um novo idioma

1. Copiar `src/i18n/locales/pt/` → `src/i18n/locales/<codigo>/`
2. Traduzir cada um dos cinco módulos
3. Registrar em `src/i18n/index.ts`:
   ```ts
   import { xx } from "./locales/xx/index";
   // SUPPORTED_LANGUAGES: adicionar { code: "xx", label: "...", locale: "xx-XX" }
   // resources: xx: { translation: xx }
   ```
4. Rodar `npm run i18n:check` — zero erros = paridade garantida

## Arquivos

| Arquivo | Conteúdo |
| --- | --- |
| `src/i18n/index.ts` | Init do i18next, helpers, augmentação de tipos |
| `src/i18n/locales/types.ts` | `Localized<T>` |
| `src/i18n/locales/en/*.ts` | Recurso inglês por domínio — fonte da verdade |
| `src/i18n/locales/pt/*.ts` | Recurso português por domínio |
| `scripts/check-i18n.mjs` | Validação de paridade via `tsc --noEmit` |
| `.github/workflows/ci.yml` | Passo `i18n:check` antes do build |
| `src/lib/errors.ts` | `translateError(t, err)` + hook `useErrorMessage()` |
| `src/types/ipc.ts` | `AppErrorPayload.detail` |
| `src-tauri/src/error.rs` | `detail()` + serializa `code`/`message`/`detail` |
| `src-tauri/src/lib.rs` | Menu da bandeja em inglês |
| `src/components/SettingsModal.tsx` | Seção "Language" com o seletor de idioma |

## Decisões

- **Locales modulares por domínio**: em vez de um único `en.ts`, cada área funcional tem seu
  arquivo. PRs de tradução tocam apenas o módulo relevante; erros de tipo apontam diretamente
  para o arquivo e a chave problema.
- **Idioma no `localStorage`, não em `Settings`**: idioma é apresentação do frontend; o backend
  não precisa dele. Mantê-lo fora da boundary evita round-trip e três pontos de espelhamento
  para uma escolha puramente de UI.
- **Inglês fixo na bandeja**: o menu nativo é construído uma vez no `setup_tray` (startup),
  fora do alcance do `t()` do frontend. Reconstruí-lo na troca de idioma seria custo
  desproporcional; inglês é o padrão reconhecido em apps desktop.
- **Erro traduzido por `code` + `detail`, não pelo `message` inteiro**: o `message` do backend
  é pré-formatado. Separar o `detail` técnico do prefixo deixa o prefixo localizável sem
  perder a informação de diagnóstico.
- **Tipagem estrita de chaves**: `CustomTypeOptions` + `Localized<T>` transformam erro de
  chave (typo, idioma incompleto) em erro de compilação, não em texto faltando em runtime.

## Limitação conhecida

O payload do evento `sync:error` carrega um `message` pré-formatado pelo backend, não um
`code`. A barra de status traduz o rótulo ao redor ("Last sync failed…"), mas o detalhe do
erro de sync em background aparece no idioma original. Localizá-lo exigiria carregar
`code`/`detail` no evento (mudança em `sync/engine.rs`), deixada para depois.
