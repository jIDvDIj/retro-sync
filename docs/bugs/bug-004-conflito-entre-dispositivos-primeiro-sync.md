# BUG-004 — Saves independentes de dispositivos diferentes não geram conflito no primeiro sync

**Status:** ✅ resolvido (detecção de conflito por `device_id` no primeiro sync)
**Severidade:** média (sobrescrita silenciosa do save de um dispositivo pelo de outro, com backup)
**Componente:** `src-tauri/src/sync/conflict.rs`, `src-tauri/src/sync/diff.rs`, `src-tauri/src/drive/files.rs`, `src-tauri/src/device.rs`

> **Resolução:** cada dispositivo passou a ter um **ID estável** (UUID v4 gerado uma vez e guardado
> no keyring do SO, sob a chave `retrosync_device_id`). Esse ID é estampado em `appProperties.deviceId`
> a cada upload, ao lado do nome amigável. No **primeiro sync** de um arquivo (sem manifest) presente
> nos dois lados e com mtimes divergentes, se a versão do Drive foi publicada por **outro** dispositivo,
> `decide()` devolve `Conflict` (o usuário escolhe) em vez de `DownloadWithBackup` (Drive vence cego).

---

## Descrição

Cenário com três dispositivos, batizado de "bug dos 3 dispositivos":

1. **Dispositivo A** joga o jogo `EX1`, sincroniza. O Drive fica com o save de `EX1`.
2. **Dispositivo B** joga o jogo `EX2`, sincroniza. (`EX1` no Drive permanece o de A.)
3. **Dispositivo C** também tem um save de `EX1` — progresso **independente**, feito offline, que C
   nunca sincronizou — e roda o sync.

Intuitivamente, A e C têm dois saves legítimos e divergentes do mesmo jogo: isso deveria ser um
**conflito** a ser resolvido pelo usuário. Mas, antes da correção, C simplesmente baixava a versão
de A por cima da sua (com backup local), sem perguntar nada — o save de C "sumia" da pasta ativa.

## Reprodução

1. A registra `PPSSPP`, joga `EX1`, sincroniza. O Drive recebe `EX1` com
   `appProperties.device = "A"` e `modifiedTime = T_A`.
2. C tem `EX1` local (mtime `T_C`), **sem entrada no `sync_manifest`** para esse arquivo (nunca o
   sincronizou).
3. C roda o sync. Para `EX1`, o diff monta:
   - `local_mtime_ms = Some(T_C)`
   - `drive_mtime_ms = Some(T_A)`
   - `last_synced = None` (sem manifest)
4. Antes: `decide()` cai no ramo de primeiro sync → mtimes divergem → **`DownloadWithBackup`**: a
   versão de A vence, o save de C vai para `<app_data>/backups/...` e a pasta ativa passa a ter a de A.

## Causa raiz

O `sync_manifest` é **local por dispositivo** e o `decide()` original olhava só timestamps. No
primeiro sync de um arquivo (`last_synced = None`), não há como distinguir:

- "tenho um save local que é evolução do que está no Drive" (deve seguir o Drive), de
- "tenho um save local **independente**, de progresso paralelo ao do Drive" (é conflito).

A regra conservadora escolhida para o [BUG-001](./bug-001-perda-save-primeiro-sync.md) — *Drive vence
com backup* — resolve a perda de dados (o backup protege), mas decide **automaticamente** num caso que
é genuinamente ambíguo quando os dois lados vieram de **máquinas diferentes**.

### Onde o `device_id` muda a decisão (e onde não muda)

Ponto crucial da análise: o ID de dispositivo só agrega informação no **primeiro sync** (sem
manifest). No caminho **com manifest**, os timestamps já decidem corretamente e o ID seria ruidoso:

- `(local mudou, drive **não** mudou desde o último sync)` → `Upload`. É avanço linear seguro sobre uma
  versão que o dispositivo já conhecia — **mesmo que essa versão tenha sido publicada por outro
  dispositivo**. Transformar isso em conflito geraria falso-positivo a cada "baixei de outra máquina,
  joguei e subi".
- `(ambos mudaram desde o último sync)` → já é `Conflict` por timestamp ([BUG-002](./bug-002-conflito-edicao-simultanea.md)).

Por isso a correção atua **exclusivamente** no ramo `last_synced = None`.

## Solução adotada

### 1. Identidade estável do dispositivo (`device_id`)

UUID v4 gerado na primeira execução e guardado no **keyring do SO** (chave `retrosync_device_id`,
serviço `com.retrosync.app`). Vive fora do SQLite **de propósito**: sobrevive à desinstalação do app e
à limpeza do banco — diferente do nome amigável (`device_name`, mutável, na tabela `app_settings`), que
o usuário pode renomear ou repetir entre máquinas.

```rust
// device.rs
pub fn get_or_create() -> AppResult<String> {
    let entry = entry()?;
    match entry.get_password() {
        Ok(existing) if is_valid(&existing) => Ok(existing),
        Ok(_) | Err(keyring::Error::NoEntry) => {
            let id = Uuid::new_v4().to_string();
            entry.set_password(&id)?;
            Ok(id)
        }
        Err(e) => Err(e.into()),
    }
}
```

### 2. Estampar a origem nos uploads

`appProperties` passou a carregar **dois** campos: `device` (nome, para exibição) e `deviceId` (UUID,
para a lógica). Os uploads recebem uma struct `DeviceTag { name, id }` em vez de só o nome — evita o
foot-gun de dois `Option<&str>` adjacentes trocados de ordem.

### 3. Regra no `decide()`

```rust
// conflict.rs — ramo de primeiro sync (last_synced = None), ambos existem
None => {
    if eq_within_tolerance(local, drive) {
        SyncAction::NoOp
    } else if published_by_other_device(drive_device, this_device) {
        SyncAction::Conflict
    } else {
        SyncAction::DownloadWithBackup
    }
}

/// Exige ambos os IDs conhecidos; na dúvida (algum ausente) → false (Drive-vence).
fn published_by_other_device(drive_device: Option<&str>, this_device: Option<&str>) -> bool {
    matches!((drive_device, this_device), (Some(drive), Some(this)) if drive != this)
}
```

`decide()` ganhou os parâmetros `drive_device` (vem de `DriveFile::device_id()`, lido do
`appProperties` da versão atual no Drive) e `this_device` (lido do keyring uma vez por sync e propagado
pelo `build_plan`). **Não houve migração do schema do `sync_manifest`** — a origem do Drive vem do
arquivo remoto, não do manifest.

### Tabela de decisão (primeiro sync, ambos existem, mtimes divergem)

| Origem da versão do Drive | Antes | Agora |
|---|---|---|
| **Outro** dispositivo | `DownloadWithBackup` | **`Conflict`** (usuário decide) |
| **Mesmo** dispositivo (ex.: reinstalação que perdeu o manifest) | `DownloadWithBackup` | `DownloadWithBackup` |
| Origem desconhecida (app antigo sem `deviceId`, ou keyring indisponível) | `DownloadWithBackup` | `DownloadWithBackup` |

## Degradação graciosa

Nada quebra quando a identidade é desconhecida — só não se detecta o conflito entre dispositivos:

- **Keyring indisponível** (Linux headless, Secret Service ausente): `device::current()` devolve `None`
  com aviso; o sync prossegue normalmente.
- **Arquivo do Drive sem `deviceId`** (subido por versão anterior do app): `drive_device = None`.
- Em qualquer um dos casos, `published_by_other_device` devolve `false` → comportamento idêntico ao de
  antes (`DownloadWithBackup`).

A feature "liga" sozinha conforme os uploads novos passam a estampar o `deviceId`.

## Arquivos modificados

| Arquivo | Mudança |
|---|---|
| [`src-tauri/src/device.rs`](../../src-tauri/src/device.rs) | Novo módulo: `get_or_create` (keyring, gera UUID v4) e `current` (async, degrada para `None`) |
| [`src-tauri/src/constants.rs`](../../src-tauri/src/constants.rs) | `KEYRING_DEVICE_ID_KEY = "retrosync_device_id"`; `DRIVE_APP_PROP_DEVICE_ID = "deviceId"` |
| [`src-tauri/src/drive/files.rs`](../../src-tauri/src/drive/files.rs) | `DriveFile::device_id()`; struct `DeviceTag`; `with_device` estampa `device` + `deviceId` |
| [`src-tauri/src/sync/conflict.rs`](../../src-tauri/src/sync/conflict.rs) | `decide()` recebe `drive_device`/`this_device`; helper `published_by_other_device` |
| [`src-tauri/src/sync/diff.rs`](../../src-tauri/src/sync/diff.rs) | `build_plan()` propaga `this_device_id` e extrai o `deviceId` do `DriveFile` |
| [`src-tauri/src/sync/engine.rs`](../../src-tauri/src/sync/engine.rs) | Lê o `device_id` (keyring) 1×/sync; passa ao `build_plan`; estampa nos uploads |
| [`src-tauri/src/lib.rs`](../../src-tauri/src/lib.rs) | Garante o `device_id` no startup (não fatal se o keyring falhar) |

A **boundary IPC não mudou de shape**: a struct `Conflict` já expunha `localDevice`/`driveDevice` (nomes,
para exibição); nenhum tipo Rust↔TS foi alterado. O `deviceId` adicionado ao snapshot
`sync_manifest.json` no Drive é registro/auditoria, não cruza a boundary.

## Testes

`conflict.rs` — novos casos:
- `primeiro_sync_de_outro_dispositivo_vira_conflito`
- `primeiro_sync_do_mesmo_dispositivo_mantem_drive_vence`
- `primeiro_sync_com_origem_desconhecida_mantem_drive_vence` (ambos os lados de ausência)
- `primeiro_sync_de_outro_dispositivo_mas_mtime_igual_e_noop`
- `com_manifest_origem_diferente_nao_vira_conflito` (garante que o caminho com manifest não regrediu)

`diff.rs` — integração do plano:
- `primeiro_sync_de_outro_dispositivo_vira_conflito`
- `primeiro_sync_do_mesmo_dispositivo_baixa_com_backup`

`device.rs` — `aceita_uuid_valido_e_rejeita_lixo` (validação; o keyring real não é tocado nos testes).

Suíte completa: **96 testes** Rust passando; `clippy` e `rustfmt` limpos.

## Como testar

```bash
# (no WSL) export CARGO_TARGET_DIR=$HOME/.cache/retro-sync-target
cargo test --manifest-path src-tauri/Cargo.toml conflict::
cargo test --manifest-path src-tauri/Cargo.toml diff::
```

Manual: com dois dispositivos (ou dois perfis de SO), jogar o **mesmo jogo** em cada um **sem
sincronizar entre eles**, depois sincronizar o segundo — o sync deve reportar `conflicts > 0` e
bloquear o emulador até a resolução, em vez de baixar silenciosamente.

## Relação com outros bugs

- [BUG-001](./bug-001-perda-save-primeiro-sync.md) — a correção refina o ramo de primeiro sync: quando
  a origem do Drive é desconhecida ou a mesma, segue valendo *Drive-vence-com-backup*.
- [BUG-002](./bug-002-conflito-edicao-simultanea.md) — reaproveita o `SyncAction::Conflict`, o bloqueio
  por emulador e o modal de resolução; aqui o conflito nasce no primeiro sync, não da edição simultânea.
- [BUG-003](./bug-003-troca-de-caminho-do-emulador.md) — mesma raiz conceitual: a confiabilidade do diff
  depende de identificar corretamente a origem/estado de cada versão.

## Referências

- `decide` → ramo de primeiro sync: [`src-tauri/src/sync/conflict.rs:35`](../../src-tauri/src/sync/conflict.rs)
- `published_by_other_device`: [`src-tauri/src/sync/conflict.rs:89`](../../src-tauri/src/sync/conflict.rs)
- `device::get_or_create` / `current`: [`src-tauri/src/device.rs:26`](../../src-tauri/src/device.rs)
- `DriveFile::device_id` / `DeviceTag`: [`src-tauri/src/drive/files.rs:49`](../../src-tauri/src/drive/files.rs)
- Manifest como fonte de verdade operacional: [`docs/05-sincronizacao.md`](../05-sincronizacao.md)
