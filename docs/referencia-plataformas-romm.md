# Referência: Suporte a Plataformas no RomM e o que trazer para o RetroSync

Análise do projeto [RomM](https://github.com/rommapp/romm) — gerenciador de ROMs
auto-hospedado — com foco em como ele modela e detecta **plataformas de console** e
o que desse modelo pode enriquecer o RetroSync.

---

## O que o RomM faz

### Slug canônico universal (`UniversalPlatformSlug`)

O coração do sistema é um `enum.StrEnum` Python em
`backend/handler/metadata/base_handler.py` (linha 318) com **mais de 460 slugs**,
cada um representando um console ou plataforma:

```python
class UniversalPlatformSlug(enum.StrEnum):
    PSX    = "psx"      # PlayStation 1
    PS2    = "ps2"
    PSP    = "psp"
    NES    = "nes"
    SNES   = "snes"
    GBA    = "gba"
    N64    = "n64"
    DC     = "dc"       # Dreamcast
    SATURN = "saturn"
    # ... 450+ entradas
```

Esses slugs são o **identificador universal**: pasta no filesystem, chave no banco
de dados e referência em todos os mapeamentos externos partem deles.

---

### Modelo de dados de uma plataforma

`backend/models/platform.py` — cada plataforma detectada vira uma linha no banco:

| Campo | Tipo | Significado |
|---|---|---|
| `slug` | str | Slug canônico (= valor do UPS) |
| `fs_slug` | str | Nome real da pasta no disco (pode diferir do slug via binding) |
| `name` | str | Nome oficial ("PlayStation Portable") |
| `custom_name` | str\|None | Apelido definido pelo usuário |
| `category` | str\|None | "Console", "Portable Console", "Arcade", etc. |
| `generation` | int\|None | Geração (5, 6, 7…) |
| `family_name` | str\|None | "Sony", "Nintendo", "Sega" |
| `igdb_id` | int\|None | ID no IGDB |
| `ss_id` | int\|None | ID no ScreenScraper |
| `ra_id` | int\|None | ID no RetroAchievements |
| `launchbox_id` | int\|None | ID no LaunchBox |
| `moby_id` | int\|None | ID no MobyGames |
| `hasheous_id` | int\|None | ID no Hasheous |
| `tgdb_id` | int\|None | ID no TheGamesDB |
| `libretro_slug` | str\|None | Nome do sistema no Libretro/RetroAchievements |
| `url_logo` | str\|None | URL do logo da plataforma |

---

### Detecção de plataformas

**Passo 1 — Varredura do filesystem**

`FSPlatformsHandler.get_platforms()` lê os subdiretórios da pasta de ROMs.
Suporta duas estruturas:

```
# Estrutura A (padrão):
library/roms/{platform_slug}/

# Estrutura B (Batocera/EmulationStation):
library/{platform_slug}/roms/
```

**Passo 2 — Resolução do slug**

Em `backend/handler/scan_handler.py`, função `scan_platform()`:

```python
if fs_slug in PLATFORMS_BINDING:       # alias do config.yml
    slug = PLATFORMS_BINDING[fs_slug]  # ex.: "gc" → "ngc"
elif fs_slug in PLATFORMS_VERSIONS:    # sub-versão
    slug = PLATFORMS_VERSIONS[fs_slug] # ex.: "naomi" → "arcade"
else:
    slug = fs_slug                     # nome da pasta = slug
```

O `config.yml` permite ao usuário mapear pastas customizadas para slugs canônicos:

```yaml
system:
  platforms:
    ps1: psx      # pasta "ps1" → slug "psx"
    gc:  ngc      # pasta "gc"  → slug "ngc"
```

**Passo 3 — Consulta a 10 provedores de metadados**

Após resolver o slug, o RomM consulta todos em paralelo:
IGDB, ScreenScraper, RetroAchievements, LaunchBox, MobyGames, Hasheous,
TheGamesDB, Flashpoint, HowLongToBeat e Libretro Thumbnails.

Se nenhum provedor responde, a plataforma é registrada como "não identificada"
mas ainda aparece no catálogo.

---

### Organização de saves **por plataforma** (não por emulador)

`backend/handler/filesystem/assets_handler.py` (linha 95):

```
{ASSETS_BASE_PATH}/users/{user_hex}/
  saves/{platform_slug}/{rom_id}/{emulator}/
  states/{platform_slug}/{rom_id}/{emulator}/
  screenshots/{platform_slug}/{rom_id}/
```

O emulador aparece **dentro** da pasta da plataforma, não como raiz.
Consequência direta: trocar de emulador não muda o caminho dos saves.

O modelo `Save` também guarda:
- `emulator`: qual emulador gerou o save (ex.: `"ppsspp"`)
- `slot`: slot numerado (suporte a histórico por slot)
- `content_hash`: MD5 para deduplicação
- `origin_device_id`: dispositivo que gerou o save (multi-dispositivo)

---

### Mapeamento emulador/core → plataforma (frontend)

`frontend/src/utils/index.ts` (linha 450) — mapeamento estático usado pelo
player EmulatorJS no browser:

```typescript
const _EJS_CORES_MAP: Record<string, string[]> = {
  psp:     ["ppsspp"],
  psx:     ["pcsx_rearmed", "mednafen_psx_hw"],
  ps2:     ["pcsx2"],          // apenas nightly
  gba:     ["mgba"],
  gbc:     ["gambatte", "mgba"],
  n64:     ["mupen64plus_next", "parallel_n64"],
  snes:    ["snes9x"],
  genesis: ["genesis_plus_gx"],
  saturn:  ["yabause"],
  arcade:  ["mame2003", "mame2003_plus", "fbneo"],
  // ... ~60 plataformas no total
};
```

A direção é `plataforma → lista de cores` — o inverso do que o RetroSync usaria
(emulador → slug da plataforma), mas a ideia de ter um mapeamento explícito é a mesma.

---

### Identificação especial por nome de arquivo

Para consoles Sony, o RomM detecta seriais no nome do arquivo via regex:

```python
SONY_SERIAL_REGEX = re.compile(r".*([a-zA-Z]{4}-\d{5}).*$")
# Detecta: SLUS-01234, SLES-01234, SCUS-01234, NPJB-00001, etc.
```

Fixtures JSON pré-compiladas (carregadas no Redis na inicialização):
- `fixtures/ps1_serial_index.json`
- `fixtures/ps2_serial_index.json`
- `fixtures/psp_serial_index.json`
- `fixtures/mame_index.json`
- `fixtures/scummvm_index.json`

Para Nintendo Switch: detecta Title IDs (`70[0-9]{12}`) e Product IDs
(`0100[0-9A-F]{12}`) — consulta o TitleDB atualizado periodicamente.

---

## O que trazer para o RetroSync

### 1. `platform_slug` no `profiles.toml` (não-breaking)

Campo declarativo adicionado a cada `[[emulator]]`:

```toml
[[emulator]]
name = "PPSSPP"
platform_slug = "psp"
# ... resto igual

[[emulator]]
name = "PCSX2"
platform_slug = "ps2"
# ... resto igual
```

**Impacto**: zero — campo opcional; não muda o comportamento do sync.  
**Habilita**: display name do console na UI, agrupamento por plataforma, base para migração futura.

### 2. Módulo de display names (escopo mínimo)

Em vez de 460 entradas como no RomM, manter apenas os slugs efetivamente usados
nos perfis. Exemplo em Rust:

```rust
pub struct PlatformInfo {
    pub slug:       &'static str,
    pub name:       &'static str,  // "PlayStation Portable"
    pub short_name: &'static str,  // "PSP"
    pub family:     &'static str,  // "Sony"
}

pub fn lookup(slug: &str) -> Option<&'static PlatformInfo> { ... }
```

Adicionar entradas conforme novos emuladores entram nos perfis. Não pré-popular
com 460 slugs sem uso — crescimento orgânico.

### 3. Expor `platform_slug` e `platform_name` na boundary IPC

O registro de emulador que o frontend recebe ganharia dois campos:

```rust
// commands.rs
pub struct EmulatorRecord {
    pub name:          String,
    pub platform_slug: Option<String>,  // "psp"
    pub platform_name: Option<String>,  // "PlayStation Portable"
    // ... campos existentes
}
```

```ts
// src/types/ipc.ts
interface EmulatorRecord {
  name:          string;
  platformSlug?: string;
  platformName?: string;
  // ...
}
```

A UI passaria a mostrar "PlayStation Portable" em vez de "PPSSPP" como título
do card, com o nome do emulador como detalhe secundário.

### 4. RetroArch (futuro)

RetroArch é multi-plataforma: o mesmo executável roda dezenas de consoles via
cores. O RomM lida com isso pelo `LIBRETRO_PLATFORM_LIST` — mapa `slug → nome_do_sistema_libretro`.

Para o RetroSync, ao adicionar RetroArch ao catálogo, seria necessário um campo
diferente no perfil: `platform_slugs_by_core` (mapa core → slug), já que um
único perfil de emulador cobriria N plataformas.

---

## Estrutura do Drive por plataforma — tradeoffs

**Situação atual:**
```
RetroSync/
  PPSSPP/
    saves/
    savestates/
  PCSX2/
    saves/ (memcards)
    savestates/
```

**Alternativa (estilo RomM):**
```
RetroSync/
  psp/
    saves/
    savestates/
  ps2/
    saves/
    savestates/
```

| Aspecto | Atual (por emulador) | Alternativa (por plataforma) |
|---|---|---|
| Troca de emulador | Perde continuidade do sync | Saves migram automaticamente |
| Multi-emulador | Saves separados por app | Saves unificados por console |
| Impacto em usuários existentes | — | **Breaking** — manifest aponta para pasta antiga |
| Complexidade | Baixa | Requer migração assistida |

**Decisão recomendada**: não migrar agora. Implementar `platform_slug` no perfil e
na UI (itens 1–3 acima) sem alterar a estrutura do Drive. Quando o catálogo de
emuladores crescer, propor migração com script de movimentação no Drive e
re-mapeamento do manifest SQLite.

---

## Arquivos de referência no RomM

| Arquivo | Conteúdo relevante |
|---|---|
| `backend/handler/metadata/base_handler.py:318` | `UniversalPlatformSlug` — enum completo |
| `backend/models/platform.py` | Model `Platform` — todos os campos |
| `backend/handler/scan_handler.py` | `scan_platform()` — detecção + `PLATFORMS_BINDING` |
| `frontend/src/utils/index.ts:450` | `_EJS_CORES_MAP` — mapeamento plataforma → cores |
| `backend/handler/filesystem/assets_handler.py:95` | Estrutura de pastas de saves |
| `backend/models/assets.py:83` | Models `Save` e `State` — campos relevantes |
| `backend/adapters/services/igdb.py:229` | `IGDB_PLATFORM_LIST` — mapa slug → ID IGDB |
| `backend/handler/metadata/ra_handler.py:470` | `RA_ID_TO_SLUG` — lookup reverso |
