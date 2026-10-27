# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

padmap mapeia qualquer controle (Xbox, PlayStation, Switch, genérico) para teclado. Workspace Rust; Windows é a plataforma principal, Linux e macOS já compilam. Código, comentários, mensagens de erro e log são em português.

## Commits

Só faça commit com aprovação explícita do usuário, pedida a cada commit. Proponha a mensagem e espere o "sim" antes de rodar `git commit`.

Mensagens de commit em inglês, no padrão Conventional Commits: `<type>(<scope opcional>): <descrição>`, com tipos como `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `ci`, `build` e `perf`. A descrição vai no imperativo, em minúsculas e sem ponto final. Use o crate como escopo quando a mudança for de um só (ex.: `feat(gui): add stick sensitivity window`). Mudanças incompatíveis levam `!` depois do tipo ou um rodapé `BREAKING CHANGE:`. Os commits anteriores a esta regra estão em português.

## Comandos

```sh
cargo run --release -p padmap-gui                 # interface gráfica (binário `padmap`)
cargo run -p padmap-cli -- list                    # controles conectados
cargo run -p padmap-cli -- monitor                 # eventos crus do controle
cargo run -p padmap-cli -- check profiles/default.toml
cargo run -p padmap-cli -- run --dry-run profiles/default.toml   # só loga as teclas

cargo test --all                                   # todos os testes
cargo test -p padmap-core mapper                   # testes de um crate filtrados por nome
```

O CI (`.github/workflows/ci.yml`, Windows/Linux/macOS) roda, e um PR precisa passar em:

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release
```

O SDL3 é compilado do fonte e linkado estático (feature `sdl-build-from-source` de `padmap-input`), então o build precisa de CMake e de um compilador C. O primeiro build é lento por isso. Para testar sem esse custo, rode só `cargo test -p padmap-core`, que não depende de SDL nem de APIs do SO.

## Arquitetura

Fluxo: `GamepadSource::next_event` (padmap-input) → `Mapper::handle` (padmap-core) → `Vec<KeyAction>` → `KeyboardSink::send` (padmap-output). `docs/arquitetura.md` explica as escolhas de stack e as decisões.

- **padmap-core** é puro: tipos `Button`/`Axis`/`InputEvent`, `Key`/`KeyCombo`, `Profile` (TOML via serde) e `Mapper`. Lógica nova de mapeamento vai aqui, com testes, e não nas camadas de SDL/SO/GUI.
- **padmap-input** traduz SDL3 para o vocabulário do core e normaliza os eixos para `f32`. Liga o hint de eventos em segundo plano porque o foco fica no jogo, não no padmap.
- **padmap-output**: trait `KeyboardSink`, escolhido em tempo de compilação por `platform_sink()`; `DryRunSink` só loga. `codes.rs` tem as três tabelas `Key` → código nativo fora dos `cfg`, para que todas compilem e sejam testadas em qualquer SO. No Windows usa scancodes (`KEYEVENTF_SCANCODE`), não teclas virtuais, porque jogos com DirectInput/Raw Input ignoram `wVk`.
- **padmap-gui** (egui/eframe): `worker.rs` roda a leitura do controle e o `Mapper` numa thread própria e fala com a interface por canais `Command`/`Status`. Isso tem de continuar assim: a janela do egui só redesenha quando algo muda e para sem foco, que é quando o jogo está na frente. A interface só desenha e edita o `Profile`, e manda o perfil inteiro com `Command::SetProfile`. `diagram.rs` desenha o controle num espaço fixo de 1000×640 escalado para a janela; `slots.rs` define `Slot` (botão ou sentido de eixo), a unidade que recebe uma tecla.
- **padmap-cli**: o mesmo fluxo, rodando direto no terminal.

## Interface: modo claro e escuro

Toda mudança de layout ou de cor na interface precisa funcionar nos dois temas. Pegue as cores da `Palette` do tema atual (`theme::palette`, com `LIGHT` e `DARK` em `padmap-gui/src/theme.rs`) em vez de escrever `Color32` direto no código. Uma cor nova que muda com o tema entra nas duas paletas. As constantes compartilhadas (`ACCENT`, `ACTIVE`, `GOOD`, `BAD`) valem para os dois temas. Cor fixa só vale onde o fundo não muda com o tema: o desenho do controle (as constantes do topo de `diagram.rs`), que é escuro nos dois modos, e texto branco sobre botões de destaque. O que muda com o tema é o fundo, as barras e as etiquetas livres. Antes de dar a mudança por pronta, confira nos dois temas (botão sol/lua no topo). As capturas em `docs/` (`tela-principal.png` e `tela-principal-escura.png`) mostram como cada tema deve ficar.

## Invariantes que cruzam arquivos

- **Nenhuma tecla pode ficar presa no SO.** O `Mapper` conta referências por tecla (dois botões ligados a `Shift` só soltam quando os dois forem soltos) e solta tudo com `release_all`/`set_profile`. O worker chama `release_all` ao desativar, ao trocar a simulação, ao desconectar o controle e ao sair. Qualquer novo caminho que mude o perfil, o sink ou o estado ativo precisa soltar as teclas antes.
- **Histerese nos eixos:** pressiona em `threshold` (por eixo ou `settings.axis_threshold`) e solta abaixo de `threshold * release_ratio`. Y negativo é para cima.
- **Layout posicional:** perfis usam `south/east/west/north`, nunca A/B/X/Y, para valer em qualquer controle.
- **Adicionar uma `Key`:** entrada na macro `keys!` em `padmap-core/src/key.rs` (nome canônico e apelidos), código nas três funções de `padmap-output/src/codes.rs` (os `match` são exaustivos e um teste exige códigos únicos por plataforma) e, se der para capturar pelo teclado, em `capture::from_egui` no GUI.
- **Adicionar um `Button`/`Axis`:** `Button::ALL` tem tamanho fixo, `padmap-input` precisa mapear o valor do SDL e o GUI precisa de rótulo e posição em `slots.rs` e `diagram.rs`.
- A interface começa com a simulação ligada (`dry_run: true` no worker). O sink nativo só é criado na primeira tecla real; se falhar, o mapeamento é desativado e o erro volta como `Status::Error`.

## Permissões por SO

Windows: nada especial, mas para mandar teclas a um jogo rodando como administrador o padmap também precisa rodar como administrador. Linux: escrita em `/dev/uinput`. macOS: permissão de Acessibilidade.
