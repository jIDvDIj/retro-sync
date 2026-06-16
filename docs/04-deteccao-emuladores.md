# 04 — Detecção de Emuladores

**Commit**: `d5a1da3` — *feat: detecção automática de emuladores PPSSPP e PCSX2*

## Objetivo

Dada a pasta raiz selecionada pelo usuário, identificar automaticamente qual emulador é
e mapear onde ficam saves, savestates e configurações — sem o usuário precisar informar.

## Arquivos

| Arquivo | Conteúdo |
| --- | --- |
| `emulator/mod.rs` | `EmulatorProfile`, `detect_emulator()` e os testes |
| `emulator/ppsspp.rs` | Perfil e marcadores do PPSSPP |
| `emulator/pcsx2.rs` | Perfil e marcadores do PCSX2 |

## `EmulatorProfile`

```rust
pub struct EmulatorProfile {
    pub name: String,             // "PPSSPP" / "PCSX2" — também o nome da pasta no Drive
    pub root_path: PathBuf,       // pasta raiz selecionada pelo usuário
    pub saves_paths: Vec<PathBuf>,   // relativos a root_path
    pub config_paths: Vec<PathBuf>,  // relativos a root_path
    pub state_paths: Vec<PathBuf>,   // relativos a root_path
}
```

Os caminhos são **relativos** à raiz — é o que permite ao SyncEngine montar a estrutura
espelhada no Drive sem conhecer o emulador. Cruza a boundary (espelhado em
`src/types/ipc.ts`).

## Estratégia de detecção

Detecção por **marcadores de filesystem**, não por executável — o usuário aponta a *pasta
de dados*, que muitas vezes nem contém o `.exe`.

### PPSSPP

Procura uma subpasta `PSP/` contendo ao menos uma de `SAVEDATA`, `PPSSPP_STATE`, `SYSTEM`.
Reconhece duas variantes:

| Variante | Local do `PSP/` | Exemplo |
| --- | --- | --- |
| Pasta de dados | `PSP/` direto na raiz | `Documents/PPSSPP` |
| Instalação portátil | `memstick/PSP/` | pasta com `memstick.ini` |

Mapeamento: `saves → PSP/SAVEDATA`, `config → PSP/SYSTEM`, `savestates → PSP/PPSSPP_STATE`
(ajustados para `memstick/PSP/...` na variante portátil).

### PCSX2

Exige `inis/` **mais** ao menos uma de `memcards/`, `sstates/`, `bios/`. Exigir duas
pastas evita falso positivo numa pasta qualquer que só tenha `inis/`.

Mapeamento: `saves → memcards`, `config → inis`, `savestates → sstates`.

## Nomes de processo (para o Passo 6)

Já declarados como constantes nos perfis, prontos para o watcher:

| Emulador | Nomes de processo |
| --- | --- |
| PPSSPP | `PPSSPPWindows64.exe`, `PPSSPPWindows.exe`, `PPSSPPSDL` |
| PCSX2 | `pcsx2-qt.exe`, `pcsx2-qtx64.exe`, `pcsx2-qtx64-avx2.exe`, `pcsx2.exe`, `pcsx2-qt` |

## Comando exposto

| Comando | Assinatura | Descrição |
| --- | --- | --- |
| `detect_emulator` | `(path: String) -> Option<EmulatorProfile>` | `None` = pasta válida, mas sem emulador reconhecido; `Err` = pasta inexistente |

O I/O de disco roda em `spawn_blocking` para não travar o runtime async.

> No Passo 5 surgiu o comando `add_emulator`, que detecta **e** persiste o perfil para
> sincronização. Ver [05 — Sincronização](./05-sincronizacao.md#comandos-expostos).

## Testes

8 testes com `tempfile` (árvores de diretório reais):

- detecta PPSSPP em pasta de dados;
- detecta PPSSPP em instalação portátil;
- não detecta PPSSPP com `PSP/` sem marcadores;
- detecta PCSX2 em pasta de dados;
- detecta PCSX2 só com `inis` + `bios`;
- não detecta PCSX2 só com `inis`;
- não detecta em pasta vazia;
- serialização em camelCase (contrato com o frontend).

## Extensibilidade

Adicionar um emulador novo (RetroArch, Dolphin, …) é: criar `emulator/<novo>.rs` com a
função `detect()` e os mapeamentos, e encadeá-lo em `detect_emulator()`. Nada no
SyncEngine muda — ele é agnóstico.
