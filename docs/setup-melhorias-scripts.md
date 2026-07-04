# Setup pendente — melhorias de tooling (scripts do Syncthing)

As melhorias descritas em [melhorias-scripts-syncthing.md](./melhorias-scripts-syncthing.md)
foram implementadas. Este documento é o **passo a passo das configurações manuais que ainda
faltam** para tudo funcionar de ponta a ponta.

## O que foi implementado e onde

| #   | Melhoria                        | Status                    | Onde                                                                       |
| --- | ------------------------------- | ------------------------- | -------------------------------------------------------------------------- |
| 1   | Hook de validação de commit     | ✅ Implementado           | `scripts/git-hooks/commit-msg` + `scripts/install-hooks.sh`                |
| 2   | Versão semântica automática     | ✅ Já existia             | `release.yml` (`mathieudutour/github-tag-action`) — nada a fazer            |
| 3   | Release notes automáticas       | ✅ Implementado           | `relnotes/` + step "Montar corpo da release" no `release.yml`               |
| 4   | Arquivo AUTHORS                 | ✅ Implementado (script)  | `scripts/update-authors.sh` + `.mailmap` + `AUTHORS`; seção "Sobre" na UI pendente |
| 5   | Lint e extração de i18n         | ✅ Implementado           | `eslint.config.js` (`i18next/no-literal-string`) + `scripts/extract-i18n.mjs` (`npm run i18n:extract`) |
| 6   | Builds reproduzíveis            | ✅ Implementado           | `SOURCE_DATE_EPOCH` nos jobs `android` e `release` do `release.yml`         |
| 7   | Cobertura de testes             | ✅ Implementado           | Job `coverage` no `ci.yml` (cargo-tarpaulin → Codecov) + badge no `README.md` |
| 8   | Verificação de licenças         | ✅ Implementado           | `src-tauri/deny.toml` + job `licenses` no `ci.yml` + `scripts/check-licenses.sh` |
| 9   | Drop de privilégios no startup  | ⏳ Não implementado       | Futuro — exige testes no Windows nativo (ver [seção 8](#8-itens-fora-desta-rodada)) |

---

## 1. Rodar `npm install` no Windows (obrigatório)

O `package.json`/`package-lock.json` ganharam a devDependency `eslint-plugin-i18next`,
mas o pacote ainda **não está no `node_modules` do lado Windows** (foi adicionado com
`--package-lock-only` para não corromper os binários nativos compartilhados). Sem isso,
`npm run lint` falha com `Cannot find module 'eslint-plugin-i18next'`.

No **PowerShell** (Windows nativo):

```powershell
npm install
```

> Se depois for buildar no WSL, aplique o fix recorrente:
> `npm install --no-save @rollup/rollup-linux-x64-gnu`.

## 2. Instalar o hook de commit (por clone/máquina)

Já instalado **neste clone** (o `.git/` é compartilhado entre Windows e WSL, então vale
para os dois lados). Em clones novos ou outras máquinas:

```bash
sh scripts/install-hooks.sh
```

O hook rejeita commits fora do padrão `tipo(escopo): descrição`
(tipos: `feat|fix|docs|chore|refactor|test|style|perf|ci|build|merge`; merges e reverts
do git passam direto).

## 3. Configurar o Codecov (para a cobertura aparecer)

O job `coverage` do CI roda os testes Rust (117 hoje) e gera o relatório mesmo sem
configuração — mas o **upload** para o Codecov precisa do token:

1. Acesse [app.codecov.io](https://app.codecov.io) e faça login com a conta GitHub.
2. Ative o repositório `jIDvDIj/retro-sync` e copie o **Upload token**.
3. No GitHub: **Settings → Secrets and variables → Actions → New repository secret**
   - Nome: `CODECOV_TOKEN`
   - Valor: o token copiado.

Sem o token o CI **não quebra** (`fail_ci_if_error: false`) — só não publica a cobertura.
O badge no `README.md` passa a renderizar após o primeiro upload bem-sucedido.

## 4. Validar o cargo-deny localmente (opcional — o CI já roda)

O `src-tauri/deny.toml` foi calibrado com o inventário real de licenças da árvore de
dependências (`cargo metadata`), então o job `licenses` do CI deve passar de primeira.
Para rodar o mesmo check localmente, instale a ferramenta (comando manual):

```bash
cargo install cargo-deny --locked
sh scripts/check-licenses.sh
```

Se um novo crate trouxer licença fora da lista, o check falha — a decisão é explícita:
adicionar a licença ao `allow` do `deny.toml` (se compatível com distribuição) ou trocar
o crate.

## 5. ~~Zerar as strings hardcoded e promover a regra i18n para `error`~~ ✅ Feito

A regra `i18next/no-literal-string` já está em **`error`**: qualquer string hardcoded em
JSX quebra a CI do PR. "RetroSync" (marca, não se traduz) está na lista de exceções em
`eslint.config.js` — a lista **estende** os excludes default do plugin (pontuação,
ALL_CAPS, entidades HTML, emoji), que precisam ser preservados porque a regra faz spread
raso das opções.

Se um lint acusar uma string nova:

- **Texto de UI** → criar chave nos locales `en`/`pt` e usar `t("...")`.
- **Marca/termo técnico que não se traduz** → adicionar ao `words.exclude` da regra,
  com critério.

Auditoria de chaves órfãs/faltando a qualquer momento: `npm run i18n:extract`.

## 6. ~~Escrever relnotes manuais para a próxima release~~ ✅ Feito (v0.5)

O [`relnotes/v0.5.md`](../relnotes/v0.5.md) já está pronto para a próxima release
(a última tag é `app-v0.4.0` e há `feat` pendente → bump minor). Para as seguintes:
antes de mergear na `main` o PR que gera a versão `X.Y.*`, criar `relnotes/vX.Y.md`
(formato em [relnotes/README.md](../relnotes/README.md)). Sem o arquivo, a release sai
só com as notas automáticas da API do GitHub (lista de PRs e contribuidores).

## 7. ~~Verificar a reprodutibilidade do build~~ ✅ Validado

O `release.yml` fixa `SOURCE_DATE_EPOCH` no timestamp do commit. **Validado em
04/07/2026 (WSL)**: dois builds do frontend no mesmo commit produziram hashes SHA-256
idênticos (`26aa31f4…`). Para repetir a validação no futuro:

```bash
export SOURCE_DATE_EPOCH=$(git log -1 --pretty=%ct)
npm run build && find dist -type f -exec sha256sum {} \; | sort | sha256sum
rm -rf dist
npm run build && find dist -type f -exec sha256sum {} \; | sort | sha256sum
# os dois hashes finais devem ser iguais
```

O `vite.config.ts` não usa `Date.now()` em banners, então nenhuma mudança de código foi
necessária.

## 8. Manter AUTHORS e .mailmap

- Quando entrar contribuidor novo: `sh scripts/update-authors.sh` e commitar o `AUTHORS`
  atualizado.
- Se alguém commitar com nome/e-mail variante (ex.: nome de usuário do GitHub), adicionar
  a identidade canônica no `.mailmap` — o script unifica automaticamente.
- Evolução futura: rodar o script no CI e commitar quando houver mudança (padrão Syncthing).

## 9. Proteção de branch: exigir apenas o check `ci-passed`

O CI tem um job **gatekeeper** (`ci-passed`) que agrega o resultado de todos os outros
jobs (issue #73). Para ele valer como porteiro:

1. No GitHub: **Settings → Branches → regra da `main`** (criar se não existir).
2. Marcar **Require status checks to pass before merging**.
3. Selecionar **somente** `ci-passed` como required check (remover os demais, se
   estiverem listados).

> O check só aparece na lista de seleção depois da **primeira execução** do workflow
> com o job — abra o PR primeiro, configure depois. A partir daí, adicionar/remover
> jobs do CI exige atualizar apenas o `needs` do `ci-passed`, sem mexer no GitHub.

## 10. Reativar o build e os checks Android (desabilitados em 04/07/2026)

Os secrets de assinatura do Android ainda não existem no GitHub, então dois jobs estão
com `if: false`:

- **`android` no `release.yml`** — a validação de secrets abortava o job a cada push na
  `main` (era a causa das falhas do workflow Release).
- **`android-check` no `ci.yml`** — desabilitado junto, por decisão, até o fluxo mobile
  voltar (o bug original dele — `cargo ndk` sem `working-directory: src-tauri` — já foi
  corrigido no próprio job).

Passos para reativar:

1. Criar os secrets no GitHub (**Settings → Secrets and variables → Actions**):
   `ANDROID_KEYSTORE_BASE64`, `ANDROID_STORE_PASSWORD`, `ANDROID_KEY_PASSWORD`.
2. Remover o `if: false` do job `android` (`release.yml`) e do `android-check` (`ci.yml`).
3. Devolver `android-check` à lista `needs` do job `ci-passed` no `ci.yml`.

### Ignores do cargo-audit (revisar periodicamente)

O job `audit` ignora `RUSTSEC-2026-0194` e `RUSTSEC-2026-0195` (`quick-xml` <0.41,
transitivo via `plist 1.9.0` e `tauri-winrt-notification 0.7.2` — parents exigem <0.41,
sem correção possível por `cargo update`; o XML processado não é controlado por
atacante). Quando esses parents atualizarem o `quick-xml`, rode
`cargo update --manifest-path src-tauri/Cargo.toml` e remova os `--ignore` do `ci.yml`.

## Itens fora desta rodada

| Item | Motivo | Próximo passo |
| --- | --- | --- |
| **#9 — Drop de privilégios no startup (Windows)** | Esforço médio; mexe em runtime, exige testes no Windows nativo | Detectar se roda como admin e re-executar com `runas /trustlevel:0x20000` ao registrar autostart |
| **#4 (parte UI) — Seção "Sobre" no SettingsModal** | É feature de UI, não de tooling | Exibir versão (`__APP_VERSION__` via `define` do Vite), link do repositório e créditos do `AUTHORS` |
| **THIRD_PARTY_LICENSES.txt no instalador** | Evolução do #8 (requisito legal p/ distribuição pública) | Gerar com `cargo-about` no build e embutir no bundle |
| **SLSA provenance no release (issue #74)** | Repo privado — attestations descartadas em [distribuicao-publica.md](./distribuicao-publica.md) (verificação exige acesso de leitura ao repo) | Quando o repo virar público: `actions/attest-build-provenance` + permissions `id-token`/`attestations` no `release.yml` |
