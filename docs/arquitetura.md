# Arquitetura

## Stack

| Peça | Escolha | Por quê |
|---|---|---|
| Linguagem | Rust | binário único e sem runtime, acesso direto às APIs nativas, compila para os três sistemas |
| Entrada | SDL3 (crate `sdl3`, compilado estático) | base de mapeamentos `gamecontrollerdb`, drivers HIDAPI para DualShock 4/DualSense/Switch Pro, XInput e Windows.Gaming.Input no Windows. Um só código para qualquer controle |
| Saída | uma implementação por SO | não existe API portátil para injetar teclas |
| Perfis | TOML (`serde`) | legível e editável à mão |
| Interface | egui/eframe | Rust puro, um binário só, roda igual nos três sistemas |

## Crates

```
crates/
  padmap-core    tipos (Button, Axis, Key), perfis TOML, sticks, touchpad, botões
                 extras e o Mapper. Sem SDL nem SO; 100% testável.
  padmap-input   GamepadSource: SDL3 -> InputEvent normalizado.
  padmap-output  trait OutputSink + backends:
                   windows.rs  SendInput com scancodes (KEYEVENTF_SCANCODE)
                   linux.rs    teclado virtual via /dev/uinput (X11 e Wayland)
                   macos.rs    CGEventPost (exige permissão de Acessibilidade)
                 codes.rs tem as três tabelas Key -> código nativo.
  padmap-gui     interface gráfica (egui), binário `padmap`. Uma thread lê o
                 controle e roda o Mapper; a janela só desenha e edita o perfil.
  padmap-cli     binário de linha de comando `padmap-cli` (list, monitor, check, run).
```

Fluxo: `GamepadSource::next_event` -> `Mapper::handle` -> `Vec<KeyAction>` -> `OutputSink::send`.

## Decisões

- **Layout posicional.** O perfil fala em `south/east/west/north`, não em
  A/B/X/Y ou Cruz/Círculo. Assim o mesmo perfil vale para Xbox, PS e Nintendo.
- **Scancodes no Windows.** Jogos que leem DirectInput/Raw Input ignoram
  teclas virtuais (`wVk`); scancodes funcionam em jogos e em apps comuns.
- **Contagem de referência.** Dois botões ligados à mesma tecla só a soltam
  quando ambos forem soltos.
- **Histerese nos eixos.** Pressiona em `threshold`, solta em
  `threshold * release_ratio`.
- **Sticks por ângulo.** Cada stick é um círculo dividido em setores
  (`padmap-core/src/stick.rs`), cada um com sua tecla. Além da histerese no
  raio, há 3° de histerese no ângulo: um setor só aciona 3° para dentro da
  borda e só solta 3° para fora, então a divisa não pisca nem aciona dois
  setores juntos. Os eixos soltos dos sticks em perfis antigos são convertidos
  em setores ao carregar.
- **Stick como mouse.** No modo `mouse` o stick não aciona setores: a
  inclinação, fora da zona morta, vira uma força de 0 a 1 que passa pela curva
  (`força^curve`) e multiplica a velocidade. Como o stick parado inclinado não
  gera eventos, o movimento é por tempo: quem roda o `Mapper` chama
  `Mapper::tick(dt)` a cada volta do laço e manda o `(dx, dy)` ao
  `OutputSink::move_mouse`. As frações de pixel se acumulam entre os ticks e o
  `dt` tem teto de 50 ms, para o cursor não saltar depois de uma pausa. Com um
  stick em modo mouse fora da zona morta o laço espera 4 ms em vez de 10 ms
  (cerca de 250 movimentos por segundo). O movimento é relativo nos três
  sistemas (`MOUSEEVENTF_MOVE`, `REL_X`/`REL_Y` no uinput); no macOS o CGEvent
  exige posição absoluta, então a posição atual é lida e somada ao
  deslocamento, com os campos de delta preenchidos para os jogos.
- **Sem DirectInput no Windows.** Ao procurar controles, o DirectInput abre
  dispositivos HID que não são controles, e microfones USB com botão de mudo
  (que têm uma interface "HID-compliant headset") paravam de mandar áudio
  enquanto o padmap estava aberto. Ele fica desligado
  (`SDL_JOYSTICK_DIRECTINPUT=0`) e o Windows.Gaming.Input, que o SDL deixa
  desligado por padrão, cobre os controles genéricos. A variável de ambiente de
  mesmo nome vale mais que o padrão do padmap.
- **Eventos em segundo plano.** O hint `SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS`
  é ligado porque o foco está sempre no jogo, não no padmap.
- **Mapeamento fora da interface.** A janela do egui só é redesenhada quando
  algo muda e para quando está sem foco, então a leitura do controle e o envio
  das teclas ficam numa thread própria (`padmap-gui/src/worker.rs`).
- **Botões extras.** O SDL dá nome aos botões que o mapeamento do controle
  conhece; os outros (traseiros, turbo…) só aparecem como botões crus do
  joystick. `padmap-input` lê o mapeamento (`SDL_GetGamepadBindings`) e só
  repassa os botões crus que ele não cobre, para nenhum botão acionar duas
  vezes. No Windows, controles em modo XInput só expõem os 11 botões do Xbox;
  botões a mais só aparecem em outros modos (DirectInput, Switch).
- **Touchpad.** Os toques chegam com o dedo e a posição normalizada. O `Mapper`
  guarda onde cada dedo está e onde encostou, e o touchpad tem três modos:
  zonas (uma grade com uma tecla por zona, que fica a mesma até o dedo sair),
  mouse (o deslocamento do primeiro dedo entra no próximo `tick`) e gestos (a
  direção em que o dedo andou mais, depois de uma distância mínima). O `y` é
  multiplicado pela proporção do touchpad (2:1) para medir as duas direções na
  mesma escala.
- **SDL enxuto.** O SDL é compilado sem vídeo, áudio, render e GPU; só
  joystick, gamepad, HIDAPI e eventos.
- **Executável único no Windows.** O SDL é linkado estático e o runtime do
  Visual C++ também (`.cargo/config.toml`). O CMake ignora o `crt-static` do
  Rust, então um arquivo de toolchain (`.cargo/msvc-static-crt.cmake`) faz o
  SDL usar o mesmo runtime.

## Próximos passos

1. Mais testes com controles reais (Xbox, DualSense, Switch, genéricos).
2. Ícone na bandeja e troca de perfil por atalho no controle.
3. Perfis por controle e por jogo (detectar o processo em primeiro plano).
4. Rolagem pelo stick, turbo, macros e camadas (segurar um botão muda o mapa).
5. Assinatura do executável, MSI/winget, `.app` para macOS e pacotes Linux.
