# FEATURE-003 — Perfis-como-dados + descoberta automática + fallback manual

**Status:** ✅ implementada — `emulator/profiles.toml` + `emulator/profiles.rs`
(catálogo dirigido por dados), comandos `add_emulator_manual` e `discover_emulators`,
tipos `DiscoveredEmulator`/`DiscoverySource` espelhados no TS. Os módulos `ppsspp.rs`
e `pcsx2.rs` deixaram de existir. O texto abaixo é o plano original.
**Componentes afetados:** `src-tauri/src/emulator/`, `src-tauri/src/commands.rs`,
`src-tauri/src/error.rs`, `src/types/ipc.ts`, `src/lib/ipc.ts`, `src/`

---

## Objetivo

Permitir suportar **qualquer** emulador sem escrever código Rust novo, por três vias
complementares — da mais automática à mais manual:

1. **Perfis-como-dados** (Parte 1) — substituir os módulos hardcoded por emulador
   (`ppsspp.rs`, `pcsx2.rs`) por uma tabela declarativa (TOML embutido no binário).
   Adicionar um emulador conhecido passa a ser editar dados, não escrever um `detect()`.
   É a base técnica das outras duas vias.
2. **Descoberta automática de instalações** (Parte 3) — o sistema varre os locais
   conhecidos de cada emulador do catálogo (pastas de dados padrão + registro do
   Windows) e apresenta os encontrados como **recomendados**. O usuário só clica em
   "adicionar"; a pasta é resolvida sozinha, sem apontar nada.
3. **Fallback manual** (Parte 2) — para instalações portáteis ou emuladores fora do
   catálogo: o usuário aponta a pasta raiz (detecção por marcadores) ou descreve as
   pastas de saves / savestates / config à mão. Isso é o que entrega de fato "qualquer
   emulador".

Da perspectiva do usuário, o fluxo ideal é abrir "adicionar emulador" e já ver a lista
de emuladores instalados detectados — escolher um e pronto. Apontar pasta é o caminho
secundário, para o que a varredura não pega.

---

## Estado atual (ponto de partida)

A detecção hoje é código por emulador:

- `emulator/mod.rs` define `EmulatorProfile { name, root_path, saves_paths,
  config_paths, state_paths }` e `detect_emulator(root)` que encadeia
  `ppsspp::detect(root).or_else(|| pcsx2::detect(root))`.
- Cada perfil (`ppsspp.rs`, `pcsx2.rs`) tem constantes de pastas, `PROCESS_NAMES` e uma
  função `detect()` com a lógica de marcadores embutida.
- `process_names(name)` faz `match` sobre os nomes canônicos.

O núcleo de sync **já é agnóstico** — `SyncEngine` opera sobre caminhos e nunca conhece
PPSSPP/PCSX2. Esta feature mexe só em *como o `EmulatorProfile` é descoberto/montado*,
não em como ele é sincronizado.

A boundary IPC já existente (a manter intacta):

| Camada | Símbolo |
| --- | --- |
| Rust struct | `emulator::EmulatorProfile` (`#[serde(rename_all = "camelCase")]`) |
| TS interface | `EmulatorProfile` em `src/types/ipc.ts` |
| Comando | `detect_emulator(path)`, `add_emulator(path)` em `commands.rs` |
| Wrapper TS | `detectEmulator(path)`, `addEmulator(path)` em `src/lib/ipc.ts` |
| Erro | `AppError::EmulatorNotDetected` → code `"emulator_not_detected"` |

---

## Parte 1 — Perfis-como-dados

### 1.1 Formato dos perfis

Um arquivo `src-tauri/src/emulator/profiles.toml` embutido no binário via
`include_str!`. Cada entrada descreve um emulador conhecido de forma declarativa:

```toml
[[emulator]]
name = "PPSSPP"
process_names = ["PPSSPPWindows64.exe", "PPSSPPWindows.exe", "PPSSPPSDL"]
# Lista de candidatos a "base" relativos à raiz; o primeiro que existir é usado.
# Vazio = a própria raiz é a base.
base_candidates = ["PSP", "memstick/PSP"]
# Pelo menos um destes precisa existir (sob a base) para confirmar a detecção.
markers = ["SAVEDATA", "PPSSPP_STATE", "SYSTEM"]
saves   = ["SAVEDATA"]
states  = ["PPSSPP_STATE"]
config  = ["SYSTEM"]
# --- Descoberta automática (Parte 3) ---
# Locais padrão onde o emulador guarda dados, por SO. Cada candidato é expandido
# (placeholders abaixo) e passa pelo MESMO detect_emulator dos marcadores acima.
data_dirs.windows = ["{documents}/PPSSPP", "{localappdata}/PPSSPP"]
data_dirs.macos   = ["{home}/Library/Application Support/PPSSPP"]
data_dirs.linux   = ["{config}/ppsspp"]
# (Só Windows) Pistas no registro que confirmam que está instalado.
registry.uninstall_names = ["PPSSPP"]          # match em DisplayName das Uninstall keys
registry.app_paths       = ["PPSSPPWindows64.exe"]  # chaves sob App Paths

[[emulator]]
name = "PCSX2"
process_names = ["pcsx2-qt.exe", "pcsx2-qtx64.exe", "pcsx2-qtx64-avx2.exe", "pcsx2.exe", "pcsx2-qt"]
base_candidates = []          # base = raiz
required = ["inis"]           # TODOS precisam existir
markers  = ["memcards", "sstates", "bios"]   # e ao menos UM destes
saves    = ["memcards"]
states   = ["sstates"]
config   = ["inis"]
data_dirs.windows = ["{documents}/PCSX2", "{localappdata}/PCSX2"]
data_dirs.macos   = ["{home}/Library/Application Support/PCSX2"]
data_dirs.linux   = ["{config}/PCSX2"]
registry.uninstall_names = ["PCSX2"]
registry.app_paths       = ["pcsx2-qt.exe"]
```

> O formato precisa expressar as duas lógicas de detecção que já existem hoje:
> PPSSPP (base relocável `PSP/` ou `memstick/PSP/` + qualquer marcador) e
> PCSX2 (`inis/` obrigatória + ao menos um secundário). Daí os campos
> `base_candidates`, `required` (E lógico) e `markers` (OU lógico).
>
> Os campos `data_dirs` e `registry` alimentam a **Parte 3** e são todos `#[serde(default)]`
> — opcionais. Um perfil sem eles continua funcionando por detecção manual (Parte 2), só
> não aparece na varredura automática.

**Placeholders de `data_dirs`** (resolvidos via crate `dirs`, por SO):

| Placeholder | Resolve para (`dirs`) | Windows típico |
| --- | --- | --- |
| `{documents}` | `document_dir()` | `C:\Users\<u>\Documents` |
| `{localappdata}` | `data_local_dir()` | `C:\Users\<u>\AppData\Local` |
| `{appdata}` | `config_dir()` (Roaming) | `C:\Users\<u>\AppData\Roaming` |
| `{config}` | `config_dir()` | (Linux) `~/.config` |
| `{home}` | `home_dir()` | `C:\Users\<u>` |

### 1.2 Carregamento e detecção

```rust
// emulator/profiles.rs
#[derive(Debug, Clone, Deserialize)]
struct ProfileSpec {
    name: String,
    process_names: Vec<String>,
    #[serde(default)]
    base_candidates: Vec<String>,
    #[serde(default)]
    required: Vec<String>,
    #[serde(default)]
    markers: Vec<String>,
    saves: Vec<String>,
    states: Vec<String>,
    config: Vec<String>,
    // --- Descoberta automática (Parte 3); todos opcionais ---
    #[serde(default)]
    data_dirs: DataDirs,
    #[serde(default)]
    registry: RegistryHints,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct DataDirs {
    #[serde(default)]
    windows: Vec<String>,
    #[serde(default)]
    macos: Vec<String>,
    #[serde(default)]
    linux: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RegistryHints {
    #[serde(default)]
    uninstall_names: Vec<String>,
    #[serde(default)]
    app_paths: Vec<String>,
}

const PROFILES_TOML: &str = include_str!("profiles.toml");

/// Parseado uma vez no primeiro uso.
fn specs() -> &'static [ProfileSpec] {
    static SPECS: OnceLock<Vec<ProfileSpec>> = OnceLock::new();
    SPECS.get_or_init(|| {
        #[derive(Deserialize)]
        struct Doc { emulator: Vec<ProfileSpec> }
        toml::from_str::<Doc>(PROFILES_TOML)
            .expect("profiles.toml embutido deve ser válido")
            .emulator
    })
}
```

`detect_emulator` passa a iterar os specs em vez de chamar funções por módulo:

```rust
pub fn detect_emulator(root: &Path) -> Option<EmulatorProfile> {
    specs().iter().find_map(|spec| try_match(root, spec))
}

fn try_match(root: &Path, spec: &ProfileSpec) -> Option<EmulatorProfile> {
    // 1. Resolve a base: primeiro base_candidate existente, ou a raiz.
    let base = if spec.base_candidates.is_empty() {
        PathBuf::new()
    } else {
        spec.base_candidates
            .iter()
            .map(PathBuf::from)
            .find(|c| root.join(c).is_dir())?
    };
    let base_abs = root.join(&base);

    // 2. required: todos precisam existir.
    if !spec.required.iter().all(|d| base_abs.join(d).is_dir()) {
        return None;
    }
    // 3. markers: ao menos um (se a lista não for vazia).
    if !spec.markers.is_empty()
        && !spec.markers.iter().any(|d| base_abs.join(d).is_dir())
    {
        return None;
    }

    let join = |dirs: &[String]| dirs.iter().map(|d| base.join(d)).collect();
    Some(EmulatorProfile {
        name: spec.name.clone(),
        root_path: root.to_path_buf(),
        saves_paths: join(&spec.saves),
        config_paths: join(&spec.config),
        state_paths: join(&spec.states),
    })
}
```

`process_names` também passa a consultar os specs:

```rust
pub fn process_names(emulator_name: &str) -> Vec<String> {
    specs()
        .iter()
        .find(|s| s.name == emulator_name)
        .map(|s| s.process_names.clone())
        .unwrap_or_default()
}
```

> ⚠️ **Mudança de assinatura:** hoje `process_names` devolve `&'static [&'static str]`.
> Com perfis-como-dados os nomes deixam de ser `'static`, então retorna `Vec<String>`
> (ou `&'static` via `Box::leak` na inicialização, se preferir não tocar os chamadores).
> Conferir o uso no `watcher/` antes de escolher. Esta é a única ruptura interna; a
> boundary IPC **não muda**.

### 1.3 Dependências

Adicionar ao `src-tauri/Cargo.toml`:

```toml
toml = "0.8"
```

(`serde` e `serde_json` já são dependências.)

### 1.4 O que sai

`ppsspp.rs` e `pcsx2.rs` deixam de existir como código — viram duas entradas em
`profiles.toml`. Os testes de `mod.rs` (`detecta_ppsspp_*`, `detecta_pcsx2_*`)
permanecem **idênticos** e passam a ser a rede de segurança da migração: se eles
continuam verdes, o TOML reproduz fielmente o comportamento antigo.

---

## Parte 2 — Fallback manual

Quando `detect_emulator` devolve `None`, hoje `add_emulator` falha com
`EmulatorNotDetected`. A feature adiciona um caminho alternativo: o usuário descreve o
perfil manualmente.

### 2.1 Novo comando `add_emulator_manual`

```rust
/// Registra um emulador cujo layout o usuário informou manualmente, quando a
/// detecção automática falhou. Os caminhos vêm relativos à raiz.
#[tauri::command]
pub async fn add_emulator_manual(
    state: State<'_, AppState>,
    name: String,
    path: String,
    saves_paths: Vec<String>,
    state_paths: Vec<String>,
    config_paths: Vec<String>,
) -> AppResult<EmulatorProfile> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(/* io NotFound, como em add_emulator */);
    }
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Other("nome do emulador é obrigatório".into()));
    }

    // Valida: cada caminho relativo informado precisa existir sob a raiz, e ao
    // menos uma categoria precisa ter caminho (não registrar perfil vazio).
    let profile = build_manual_profile(&root, name, saves_paths, state_paths, config_paths)?;

    let to_store = profile.clone();
    state.db.with(move |conn| emulators::upsert(conn, &to_store)).await?;
    Ok(profile)
}
```

Pontos de atenção:

- **Validação de caminhos:** rejeitar caminhos absolutos, `..` (path traversal) e
  caminhos que não existem sob a raiz. Normalizar para relativos antes de gravar.
- **Colisão de nome:** `emulators::upsert` faz `INSERT OR REPLACE` por `name`. Para o
  fluxo manual convém checar antes e avisar a UI ("já existe um emulador com esse
  nome") em vez de sobrescrever silenciosamente. Pode ser um novo `AppError` ou
  reuso de `Other`.
- **Watcher:** um emulador manual não tem `process_names`, logo os gatilhos
  `emulator-start` / `emulator-stop` não disparam para ele. Sync `manual` e `startup`
  continuam funcionando normalmente. Documentar isso na UI. (Extensão futura: deixar o
  usuário informar o nome do processo — fora do escopo desta feature.)

### 2.2 Boundary IPC a atualizar (os três lugares)

Como `EmulatorProfile` já existe e não muda de shape, só há comando novo a espelhar:

1. **Rust** (`commands.rs`): `add_emulator_manual` + registrar no `invoke_handler` em
   `lib.rs`.
2. **`src/types/ipc.ts`**: nenhum tipo novo (reaproveita `EmulatorProfile`). Se um novo
   `code` de erro for criado para colisão de nome, atualizar o union `AppErrorPayload`.
3. **`src/lib/ipc.ts`**: wrapper novo

```ts
export function addEmulatorManual(
  name: string,
  path: string,
  savesPaths: string[],
  statePaths: string[],
  configPaths: string[],
): Promise<EmulatorProfile> {
  return invoke<EmulatorProfile>("add_emulator_manual", {
    name,
    path,
    savesPaths,
    statePaths,
    configPaths,
  });
}
```

> ⚠️ Tauri converte os parâmetros de `snake_case` (Rust) para `camelCase` no
> `invoke`. Conferir a convenção já usada pelos outros comandos (`detect_emulator`
> recebe `{ path }`) — manter consistente.

### 2.3 Fluxo de UI

1. Usuário aponta a pasta raiz → chama `detectEmulator(path)`.
2. **Detectou** (`EmulatorProfile`): mostra confirmação → `addEmulator(path)` (fluxo
   atual, inalterado).
3. **Não detectou** (`null`): a UI abre o formulário manual — campo de nome + três
   seletores de pasta (saves / savestates / config), cada um restrito a subpastas da
   raiz. Ao confirmar → `addEmulatorManual(...)`.

---

## Parte 3 — Descoberta automática de emuladores instalados

Combina dois sinais, ambos derivados do mesmo `profiles.toml`:

- **Sinal A — pasta de dados** (vale em todos os SOs): expande cada entrada de
  `data_dirs.<so>` e roda o `detect_emulator` já existente (Parte 1) sobre ela. Casou =
  achamos saves reais → recomendação **pronta para adicionar em um clique**.
- **Sinal B — registro do Windows** (só Windows): confirma que o emulador está
  *instalado* mesmo que ainda não tenha saves. Usa as `registry.uninstall_names`
  (match em `DisplayName` das chaves de Uninstall) e `registry.app_paths`. Serve para
  recomendar um emulador "instalado, sem saves ainda" e, quando o registro traz
  `InstallLocation`, para oferecer essa pasta como raiz candidata extra (cobre instalações
  que guardam dados ao lado do `.exe`).

### 3.1 Resultado da descoberta

```rust
/// Espelhado em `src/types/ipc.ts` (`DiscoveredEmulator`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredEmulator {
    /// Nome canônico do catálogo.
    pub name: String,
    /// Perfil resolvido quando os saves foram encontrados (Sinal A). `None` =
    /// instalado mas sem pasta de dados ainda (só Sinal B).
    pub profile: Option<EmulatorProfile>,
    /// De onde veio o reconhecimento.
    pub source: DiscoverySource,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiscoverySource {
    /// Pasta de dados encontrada (tem `profile`).
    DataDir,
    /// Só registro confirmou instalação (sem `profile`).
    Registry,
    /// Ambos.
    Both,
}
```

> Decisão: a descoberta **não grava nada**. Só devolve sugestões. Adicionar de verdade
> continua passando por `add_emulator(path)` (Parte 1) com a raiz resolvida — assim a
> Parte 3 não duplica a lógica de persistência nem o `upsert`.

### 3.2 Comando `discover_emulators`

```rust
/// Varre locais conhecidos + registro e devolve emuladores do catálogo
/// reconhecidos no sistema. Não persiste nada. Faz I/O de disco e (no Windows)
/// leitura de registro — roda em spawn_blocking.
#[tauri::command]
pub async fn discover_emulators() -> AppResult<Vec<DiscoveredEmulator>> {
    tokio::task::spawn_blocking(emulator::discover_installed)
        .await
        .map_err(|e| AppError::Other(format!("tarefa bloqueante abortada: {e}")))
}
```

Esboço de `discover_installed`:

```rust
pub fn discover_installed() -> Vec<DiscoveredEmulator> {
    specs().iter().filter_map(|spec| {
        // Sinal A: tenta achar saves nos data_dirs do SO atual.
        let by_data = data_dirs_for_os(spec)
            .iter()
            .filter_map(|tpl| expand_placeholders(tpl))      // {documents} -> caminho real
            .find_map(|root| detect_emulator(&root));        // reusa Parte 1

        // Sinal B: registro (no-op fora do Windows).
        let by_registry = registry_match(spec);              // bool + InstallLocation

        match (by_data, by_registry.installed) {
            (Some(profile), true)  => Some(rec(spec, Some(profile), DiscoverySource::Both)),
            (Some(profile), false) => Some(rec(spec, Some(profile), DiscoverySource::DataDir)),
            (None, true)           => Some(rec(spec, None, DiscoverySource::Registry)),
            (None, false)          => None,                  // nada reconhecido
        }
    }).collect()
}
```

> **Multiplataforma:** `registry_match` fica atrás de `#[cfg(target_os = "windows")]` e
> tem um stub `#[cfg(not(windows))]` que devolve "não instalado". Em macOS/Linux a
> descoberta funciona só com o Sinal A. O alvo de produção é Windows, mas a CI builda os
> três — o stub evita quebrar o build.

### 3.3 Dependências

Além do `toml` (Parte 1):

```toml
dirs = "5"                       # resolve {documents}/{appdata}/{home}/...

[target.'cfg(windows)'.dependencies]
winreg = "0.52"                  # leitura das Uninstall keys / App Paths
```

### 3.4 Boundary IPC a atualizar (os três lugares)

1. **Rust** (`commands.rs`): comando `discover_emulators` + registrar no `invoke_handler`
   em `lib.rs`. Tipos `DiscoveredEmulator` / `DiscoverySource` em `emulator/`.
2. **`src/types/ipc.ts`**: interface `DiscoveredEmulator` (reusa `EmulatorProfile`) e o
   union `DiscoverySource` (`"dataDir" | "registry" | "both"`).
3. **`src/lib/ipc.ts`**:

```ts
export function discoverEmulators(): Promise<DiscoveredEmulator[]> {
  return invoke<DiscoveredEmulator[]>("discover_emulators");
}
```

### 3.5 Fluxo de UI

1. Ao abrir "adicionar emulador", a UI chama `discoverEmulators()` e lista os resultados:
   - `profile != null` → botão **Adicionar** direto (chama `addEmulator(profile.rootPath)`).
   - `profile == null` (só registro) → item esmaecido: "instalado, sem saves ainda — abra
     o emulador uma vez para gerar saves" (ou link para o fluxo manual apontando a pasta
     de instalação).
2. Abaixo da lista, a opção **Apontar pasta manualmente** (Parte 2) permanece sempre
   visível — é o caminho para portáteis e emuladores fora do catálogo.

---

## Ordem de implementação sugerida

1. **Migração para perfis-como-dados** (Parte 1) sem mudar comportamento — os testes
   existentes de detecção são o critério de aceite. Refatoração pura.
2. **Comando `add_emulator_manual`** + validação de caminhos (Parte 2.1) com testes
   unitários.
3. **Descoberta automática** (Parte 3): campos `data_dirs`/`registry` no spec,
   `discover_installed` (Sinal A primeiro; Sinal B atrás de `cfg(windows)`) e o comando
   `discover_emulators`.
4. **Boundary TS** (`ipc.ts`) + fluxo de UI das três vias (Partes 2.3 e 3.5).
5. Atualizar [`referencia-ipc.md`](../referencia-ipc.md) com os comandos novos
   (`add_emulator_manual`, `discover_emulators`) e os tipos `DiscoveredEmulator` /
   `DiscoverySource`; registrar em [`decisoes-tecnicas.md`](../decisoes-tecnicas.md) a
   decisão de descoberta por locais-conhecidos + registro (não por heurística cega de
   varredura de disco inteira).

---

## Testes

- **Parte 1:** os testes atuais em `emulator/mod.rs` devem passar sem alteração após a
  migração. Adicionar um teste que confirma que `profiles.toml` parseia e contém os dois
  perfis esperados.
- **Parte 2:** `build_manual_profile` — rejeita caminho absoluto, rejeita `..`, rejeita
  caminho inexistente, rejeita perfil sem nenhuma categoria, aceita perfil válido e
  produz `EmulatorProfile` com caminhos relativos corretos.
- **Storage:** `upsert`/`list` já têm round-trip testado; um perfil manual passa pelo
  mesmo caminho, então nenhuma mudança de storage é necessária.
- **Parte 3:** `expand_placeholders` resolve cada placeholder conhecido e rejeita os
  desconhecidos; `discover_installed` num diretório temporário montado como um data_dir
  fake retorna a recomendação esperada com `source = DataDir`. O caminho de registro
  (Sinal B) é testado só no Windows (`#[cfg(windows)]`).

---

## Riscos

| Risco | Mitigação |
| --- | --- |
| TOML embutido inválido derruba o app no boot (`expect`) | Teste unitário que parseia `profiles.toml`; roda na CI antes do release. |
| Usuário aponta no manual uma pasta gigante (ROMs/BIOS) como "saves" | UI restringe seleção a subpastas da raiz + aviso de tamanho; sync continua não-destrutivo. |
| Caminho com `..` escapa da raiz | Validação rejeita `..` e caminhos absolutos antes de gravar. |
| Emulador manual não dispara watcher | Comportamento documentado na UI; sync manual/startup cobrem o caso. |
| Locais padrão (`data_dirs`) mudam entre versões do emulador | São candidatos múltiplos por SO; quando nenhum casa, a varredura só não recomenda — o fluxo de apontar pasta (Parte 2) cobre. |
| Falso positivo na descoberta (pasta de dados existe mas vazia) | Sinal A só recomenda quando `detect_emulator` confirma os marcadores; pasta sem marcadores não casa. |
| Leitura de registro indisponível/sem permissão | `registry_match` trata erro como "não instalado"; descoberta degrada para só o Sinal A. |
| `discover_emulators` lento com catálogo grande | Roda em `spawn_blocking`; é só `stat` de poucos caminhos + leitura de chaves específicas, não varredura de disco. |
