# padmap

Mapeia qualquer controle (Xbox, PlayStation, Switch ou genérico) para o
teclado. Windows primeiro, com Linux e macOS já previstos na arquitetura.

> Nome provisório. Estado: esqueleto inicial, só linha de comando.

## Como funciona

1. O SDL3 lê o controle e entrega botões e eixos num layout padrão
   (`south` é A no Xbox, Cruz no PlayStation).
2. Um **perfil** em TOML diz qual tecla cada botão ou eixo aciona.
3. O motor de mapeamento gera "pressiona/solta tecla".
4. Um backend nativo injeta as teclas: `SendInput` no Windows, `uinput` no
   Linux e `CGEvent` no macOS.

Detalhes em [docs/arquitetura.md](docs/arquitetura.md).

## Uso

```sh
cargo build --release

padmap list                              # controles conectados
padmap monitor                           # mostra os eventos ao mexer no controle
padmap check profiles/default.toml       # valida um perfil
padmap run profiles/default.toml         # mapeia de verdade
padmap run --dry-run profiles/default.toml  # só mostra as teclas no log
```

## Perfil

```toml
name = "Meu perfil"

[settings]
axis_threshold = 0.5   # quanto do curso do eixo para acionar a tecla
release_ratio = 0.8    # solta abaixo de 0.5 * 0.8 (evita "piscar")

[buttons]
south = "Space"
start = "Ctrl+S"       # combinações com +

[axes.left_x]
negative = "A"
positive = "D"

[axes.right_trigger]
positive = "LShift"
threshold = 0.3
```

Botões: `south east west north back guide start left_stick right_stick
left_shoulder right_shoulder dpad_up dpad_down dpad_left dpad_right misc1
left_paddle1 left_paddle2 right_paddle1 right_paddle2 touchpad`.

Eixos: `left_x left_y right_x right_y left_trigger right_trigger`
(Y negativo é para cima).

Teclas: letras, `0`-`9`, `F1`-`F12`, `Space Enter Escape Tab Backspace
CapsLock Insert Delete Home End PageUp PageDown Up Down Left Right`,
modificadores `Shift/LShift/RShift Ctrl/LCtrl/RCtrl Alt/LAlt/RAlt/AltGr
Win/Meta/Cmd` e pontuação (`Minus Equal Comma Period Slash ...`).

## Compilando

Precisa de Rust estável, CMake e um compilador C (o SDL3 é compilado junto,
estaticamente). No Windows, o Visual Studio Build Tools já traz os dois.

## Permissões por sistema

- **Windows:** nada especial. Para enviar teclas a um jogo rodando como
  administrador, o padmap também precisa rodar como administrador.
- **Linux:** acesso de escrita a `/dev/uinput` (ex.: regra udev ou grupo `input`).
- **macOS:** permissão de Acessibilidade para o terminal ou o app.
