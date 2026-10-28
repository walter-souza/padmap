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

Fluxo: `GamepadSource::next_event` (padmap-input) → `Mapper::handle` (padmap-core) → `Vec<KeyAction>` → `OutputSink::send` (padmap-output). `docs/arquitetura.md` explica as escolhas de stack e as decisões.

- **padmap-core** é puro: tipos `Button`/`Axis`/`InputEvent`, `Key`/`KeyCombo`, `Profile` (TOML via serde), `Stick`/`StickBinding` (sticks por setores de ângulo ou como mouse) e `Mapper`. Lógica nova de mapeamento vai aqui, com testes, e não nas camadas de SDL/SO/GUI.
- **padmap-input** traduz SDL3 para o vocabulário do core e normaliza os eixos para `f32`. Liga o hint de eventos em segundo plano porque o foco fica no jogo, não no padmap.
- **padmap-output**: trait `OutputSink` (teclas com `send`, mouse relativo com `move_mouse`), escolhido em tempo de compilação por `platform_sink()`; `DryRunSink` só loga. `codes.rs` tem as três tabelas `Key` → código nativo fora dos `cfg`, para que todas compilem e sejam testadas em qualquer SO. No Windows usa scancodes (`KEYEVENTF_SCANCODE`), não teclas virtuais, porque jogos com DirectInput/Raw Input ignoram `wVk`.
- **padmap-gui** (egui/eframe): `worker.rs` roda a leitura do controle e o `Mapper` numa thread própria e fala com a interface por canais `Command`/`Status`. Isso tem de continuar assim: a janela do egui só redesenha quando algo muda e para sem foco, que é quando o jogo está na frente. A interface só desenha e edita o `Profile`, e manda o perfil inteiro com `Command::SetProfile`. `diagram.rs` desenha o controle num espaço fixo de 1000×640 escalado para a janela; `slots.rs` define `Slot` (botão ou gatilho), a unidade que recebe uma tecla na tela principal. Os perfis do usuário ficam numa pasta própria (`profiles.rs`, `Library`: `eframe::storage_dir("padmap")/perfis`), listada no menu do nome do perfil; salvar grava ali sem perguntar, e só Importar e Exportar abrem a janela de arquivo. Os sticks são editados à parte, em `stick_editor.rs` (modal "Mapeamento do Stick", com os modos Teclas e Mouse), que trabalha numa cópia do `StickBinding` e só grava no perfil ao salvar.
- **padmap-cli**: o mesmo fluxo, rodando direto no terminal.

## Interface: modo claro e escuro

Toda mudança de layout ou de cor na interface precisa funcionar nos dois temas. Pegue as cores da `Palette` do tema atual (`theme::palette`, com `LIGHT` e `DARK` em `padmap-gui/src/theme.rs`) em vez de escrever `Color32` direto no código. Uma cor nova que muda com o tema entra nas duas paletas. As constantes compartilhadas (`ACCENT`, `ACTIVE`, `GOOD`, `BAD`) valem para os dois temas. Cor fixa só vale onde o fundo não muda com o tema: o desenho do controle (as constantes do topo de `diagram.rs`), que é escuro nos dois modos (grafite no claro, para não pesar sobre o fundo branco), e texto branco sobre botões de destaque. O que muda com o tema é o fundo, as barras, as etiquetas (no claro, as mapeadas são brancas e só o selo da tecla é azul) e as linhas de ligação. Antes de dar a mudança por pronta, confira nos dois temas (botão sol/lua no topo). As capturas em `docs/` (`tela-principal.png` e `tela-principal-escura.png`) mostram como cada tema deve ficar.

## Invariantes que cruzam arquivos

- **Nenhuma tecla pode ficar presa no SO.** O `Mapper` conta referências por tecla (dois botões ligados a `Shift` só soltam quando os dois forem soltos) e solta tudo com `release_all`/`set_profile`. O worker chama `release_all` ao desativar, ao trocar a simulação, ao desconectar o controle e ao sair. Qualquer novo caminho que mude o perfil, o sink ou o estado ativo precisa soltar as teclas antes.
- **Histerese nos eixos:** pressiona em `threshold` (por eixo ou `settings.axis_threshold`) e solta abaixo de `threshold * release_ratio`. Y negativo é para cima.
- **Sticks são setores, não eixos soltos.** Ângulos em graus, anti-horário a partir da direita. Os setores de um `StickBinding` ficam ordenados por `start` em `[0, 360)` e cobrem o círculo; edite pelos métodos dele (`move_start`, `split`, `remove`), que mantêm isso. O `Mapper` identifica um setor pelo índice, o que só é seguro porque `set_profile` solta tudo antes de trocar o perfil. Ao carregar, `axes.left_x`/`left_y`/`right_x`/`right_y` viram setores (`migrate_stick_axes`), então `axes` só guarda gatilhos e eixos usados por sticks não podem aparecer nos dois lugares.
- **Mouse é por tempo, não por evento.** No modo `mouse` (`StickMode::Mouse`) os setores do stick ficam guardados mas não acionam. O stick parado inclinado não gera eventos, então todo laço que roda um `Mapper` (o worker do GUI e o `run` da CLI) chama `Mapper::tick(dt)` a cada volta e manda o resultado a `OutputSink::move_mouse`; um laço novo que esqueça isso deixa o mouse parado. A curva fica em `MouseSettings::velocity`, usada também pela área de teste do editor.
- **Layout posicional:** perfis usam `south/east/west/north`, nunca A/B/X/Y, para valer em qualquer controle.
- **Adicionar uma `Key`:** entrada na macro `keys!` em `padmap-core/src/key.rs` (nome canônico e apelidos), código nas três funções de `padmap-output/src/codes.rs` (os `match` são exaustivos e um teste exige códigos únicos por plataforma) e, se der para capturar pelo teclado, em `capture::from_egui` no GUI. Os cliques do mouse (`MouseLeft/Right/Middle`) também são `Key`, para valerem em qualquer mapeamento e combinação: `Key::mouse_button()` os identifica, os sinks do Windows e do macOS os mandam como evento de mouse (as tabelas desses dois devolvem `None` para eles) e no Linux são `BTN_*`, que o uinput trata como tecla. Como não dá para capturá-los pelo teclado, a captura tem botões próprios (`capture::mouse_buttons_ui`).
- **Botões extras** (`Profile::extras`, aba "Botões extras" em `extras.rs`): qualquer botão fora do desenho, com nome dado pelo usuário. `ExtraSource::Known` é um botão com nome no SDL que o desenho não mostra (`slots::OFF_DIAGRAM`); `ExtraSource::Other(n)` é o botão cru `n` do joystick, que `padmap-input` só repassa (como `InputEvent::OtherButton*`) quando o mapeamento do controle não o cobre (`SDL_GetGamepadBindings`), para nenhum botão acionar duas vezes. O perfil recusa o mesmo botão em `buttons` e em `extras`.
- **Touchpad** (`Profile::touchpad`, `padmap-core/src/touchpad.rs`, editor em `touchpad_editor.rs`): uma grade de zonas com uma tecla cada. `padmap-input` repassa os toques do primeiro touchpad como `InputEvent::Touch*` (dedo, x e y em `0..=1`). O `Mapper` guarda onde está cada dedo e aciona `Source::Zone(dedo, zona)`: no modo `click`, ao apertar o touchpad, para cada dedo encostado (e para um dedo que encosta com ele já apertado, porque o SDL pode mandar o clique antes do toque); no modo `touch`, ao encostar. A zona fica a mesma até o dedo sair. No modo `click` o botão `touchpad` não pode ter tecla em `buttons` nem em `extras`; o editor tira essa tecla ao salvar e a aba de extras recusa o botão.
- **Adicionar um `Button`/`Axis`:** `Button::ALL` tem tamanho fixo, `padmap-input` precisa mapear o valor do SDL e o GUI precisa de rótulo e posição em `slots.rs` e `diagram.rs`.
- A interface começa com a simulação ligada (`dry_run: true` no worker). O sink nativo só é criado na primeira tecla real; se falhar, o mapeamento é desativado e o erro volta como `Status::Error`.

## Permissões por SO

Windows: nada especial, mas para mandar teclas a um jogo rodando como administrador o padmap também precisa rodar como administrador. Linux: escrita em `/dev/uinput`. macOS: permissão de Acessibilidade.
