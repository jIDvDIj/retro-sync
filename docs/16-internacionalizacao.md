# 16 — Internacionalização (i18n)

> Torna a interface disponível em múltiplos idiomas. Idiomas iniciais: **English** (padrão)
> e **Português (Brasil)**. A infraestrutura aceita novos idiomas sem tocar nos componentes.

## O quê

Toda string visível ao usuário no frontend passou a vir do i18n (`react-i18next`). O idioma:

- **padrão é inglês** (`en`) na primeira execução;
- é **escolhido pelo usuário** num seletor na seção "Language" do modal de configurações;
- é **persistido em `localStorage`** (`retrosync.language`) — não cruza a boundary, pois é
  concern puramente de apresentação.

O menu nativo da bandeja (Rust) também foi para inglês (`Open` / `Sync now` / `Quit`).

## Por quê

O app nasceu todo em português. Para distribuí-lo publicamente, inglês como padrão alcança o
maior público; o português continua disponível num clique. Manter os textos centralizados em
arquivos de locale (em vez de espalhados no JSX) é o que permite adicionar um terceiro idioma
sem caçar strings componente a componente.

## Como

### Infraestrutura (`src/i18n/`)

| Arquivo | Conteúdo |
| --- | --- |
| `i18n/index.ts` | `init` do i18next, `SUPPORTED_LANGUAGES`, `storedLanguage()`, `changeLanguage()`, `currentLocale()`, sync de `<html lang>`, augmentação de tipos |
| `i18n/locales/en.ts` | Recurso inglês — **fonte da verdade** da forma (`Resources = typeof en`) |
| `i18n/locales/pt.ts` | Recurso português, tipado `Localized<Resources>` |

`Localized<T>` (em `en.ts`) é a mesma forma de `Resources` mas com folhas `string`: tipar `pt`
com ele faz o **TypeScript exigir exatamente as mesmas chaves** em todos os idiomas — chave
faltante ou sobrando quebra o `npm run build`.

A inicialização é **síncrona** e os recursos são **embutidos** (sem backend HTTP, sem Suspense):
os componentes renderizam já no idioma certo. `CustomTypeOptions` (augmentação de `i18next`)
torna `t("chave")` checado em tempo de compilação — chave inexistente é erro de tipo.

### Tradução de erros do backend (`src/lib/errors.ts`)

O `AppError` do Rust serializa com `code` (enum fechado) + `message` (texto completo) + `detail`
(só o detalhe técnico, sem prefixo — campo novo, ver [Boundary](#boundary)). O frontend localiza
o prefixo pelo `code` (chaves `errors.<code>`) e anexa o `detail`:

```
errorMessage(io)  →  t("errors.io") + ": " + detail   // "I/O error: <detalhe da lib>"
```

`code: "other"` não tem prefixo a traduzir — cai no `detail`/`message` como veio. O acesso é via
hook `useErrorMessage()`, que devolve um tradutor com identidade estável (só muda quando o idioma
muda), preservando os `useCallback`/`useEffect` dos hooks de dados.

### Boundary

`AppErrorPayload` ganhou o campo `detail: string` (`error.rs` serializa 3 campos; espelhado em
`src/types/ipc.ts`). O `code` segue enum fechado: alterá-lo exige atualizar o union no `ipc.ts`
**e** as chaves `errors.*` nos locales. Ver [referencia-ipc.md](./referencia-ipc.md#apperrorpayload).

### Datas e plurais

`toLocaleString` usa `currentLocale()` (`en-US`/`pt-BR`) em vez do fixo `pt-BR`. Plurais e
interpolação ficam no i18next: chaves `*_one`/`*_other` (ex.: `sync.backupBanner`,
`emulator.resolveConflict`) e `{{count}}`/`{{when}}`.

## Arquivos

| Arquivo | Mudança |
| --- | --- |
| `src/i18n/index.ts`, `locales/en.ts`, `locales/pt.ts` | Infraestrutura i18n + locales (novos) |
| `src/main.tsx` | `import "./i18n"` antes do App |
| `index.html` | `lang="en-US"` |
| `src/lib/errors.ts` | `translateError(t, err)` + hook `useErrorMessage()` (antes: função `errorMessage`) |
| `src/types/ipc.ts` | `AppErrorPayload.detail` |
| `src-tauri/src/error.rs` | `detail()` + serializa `code`/`message`/`detail` |
| `src-tauri/src/lib.rs` | Menu da bandeja em inglês |
| `src/components/*.tsx`, `src/hooks/*.ts` | Strings → `t(...)`; `errorMessage` → `useErrorMessage()` |
| `src/components/SettingsModal.tsx` | Seção "Language" com o seletor de idioma |

## Decisões

- **Idioma no `localStorage`, não em `Settings`**: idioma é apresentação do frontend; o backend
  não precisa dele (a bandeja é inglês fixo). Mantê-lo fora da boundary evita um round-trip e
  três pontos de espelhamento para uma escolha puramente de UI.
- **Inglês fixo na bandeja**: o menu nativo é construído uma vez no `setup_tray` (startup),
  fora do alcance do `t()` do frontend. Reconstruí-lo na troca de idioma seria custo
  desproporcional; inglês é o padrão reconhecido em apps desktop. Se um dia for traduzido, o
  Rust precisaria ler o locale salvo no startup.
- **Erro traduzido por `code` + `detail`, não pelo `message` inteiro**: o `message` do backend é
  português pré-formatado. Separar o `detail` técnico do prefixo deixa o prefixo localizável sem
  perder a informação de diagnóstico (caminho, nome, mensagem da lib).
- **Tipagem estrita de chaves**: `CustomTypeOptions` + `Localized<Resources>` transformam erro de
  chave (typo, idioma incompleto) em erro de compilação, não em texto faltando em runtime.

## Limitação conhecida

O payload do evento `sync:error` (`SyncErrorEvent`) carrega um `message` pré-formatado pelo
backend (português), não um `code`. A barra de status traduz o rótulo ao redor ("Last sync
failed…"), mas o detalhe do erro de sync em background aparece no idioma original. Localizá-lo
exigiria carregar `code`/`detail` no evento (mudança em `sync/engine.rs`), deixada para depois.
