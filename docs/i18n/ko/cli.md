<!-- i18n: source=docs/cli.md commit=14af788efc95 -->
> 이 문서는 번역본입니다. 기준은 [영어 문서](../../cli.md)이며, 영어 문서가 더 최신일 수 있습니다.

# leon 명령 레퍼런스

모든 `leon` 명령과 플래그입니다. `leon <command> --help`는 같은 내용을 짧게 출력합니다. 이 문서(영어판)와 `docs/cli/`에 있는 그 하위 문서가 `--help`에 나오는 명령이나 플래그를 빠뜨리면 테스트(`crates/leon/tests/m7b_cli_doc.rs`)가 실패합니다.

## 명령이 대상을 찾는 방법

leon은 **세션**을 가집니다. 세션은 **창**(탭)을 담고, 창은 분할 레이아웃으로 **블록**(터미널 페인)을 담습니다. 모든 객체에는 `block:91c0…` 같은 변하지 않는 ID가 있습니다.

- **대상.** `-s`는 세션을, `-b`는 블록을 고릅니다. 대상은 전체 ID, 레이블(대소문자 무시), 1부터 세는 위치(창은 탭 순서로 셉니다), 또는 ID의 마지막 4자 이상으로 지정합니다. 대상이 여러 개와 일치하면 leon은 일치 항목을 나열하고 3으로 종료합니다.
- **기본값.** leon 터미널 안에서는 `LEON_SESSION`과 `LEON_BLOCK`이 내 세션과 블록을 가리키므로, 명령은 플래그 없이 "여기"에 적용됩니다. 내 세션은 지금 `LEON_BLOCK`을 담고 있는 세션이고(페인은 다른 세션으로 옮겨 갈 수 있습니다), 아니면 `LEON_SESSION`입니다. leon 밖에서는 `-s`/`-b`를 넘기세요. 이때 세션에 대한 명령은 그 세션의 활성 창에서 포커스된 블록에 적용됩니다.
- **서버.** 서버가 필요한 명령은 실행 중인 서버가 없으면 개인 서버를 시작합니다(`--autostart=false`나 `LEON_AUTOSTART=0`으로 끌 수 있습니다). 소켓은 `$LEON_RUNTIME_DIR`, 없으면 `$XDG_RUNTIME_DIR/leon`, 그것도 없으면 `~/.local/state/leon/run`에 있습니다. 그 경로는 Unix 소켓에 들어갈 길이(macOS는 104바이트 미만, Linux는 108바이트 미만)여야 하며, 아니면 leon이 그렇게 알리고 멈춥니다.
- **다른 머신.** `-H HOST`는 내 `ssh`를 통해 모든 클라이언트 명령을 다른 머신의 leon 서버로 보냅니다([cli/hosts.md](../../cli/hosts.md) 참고).

## 전역 플래그

모든 명령에서 쓸 수 있습니다.

| 플래그 | 하는 일 |
|---|---|
| `--autostart[=true\|false]` | 실행 중인 서버가 없으면 개인 서버를 시작합니다(기본값 true, 환경 변수 `LEON_AUTOSTART`). |
| `--json` | 텍스트 대신 JSON을 출력합니다. |
| `-s, --session SESSION` | 대상 세션(기본값 `$LEON_SESSION`). |
| `-b, --block BLOCK` | 대상 블록(기본값 `$LEON_BLOCK`, 없으면 활성 창에서 포커스된 블록). |
| `-H, --host HOST` | ssh를 통해 다른 머신의 leon 서버를 다룹니다: `leon hosts`의 레이블, 또는 `ssh://[USER@]HOST[:PORT]`. `-S, --server`로도 씁니다. 이를 쓰면 `$LEON_SESSION`과 `$LEON_BLOCK`은 무시됩니다(이 머신의 객체를 가리키기 때문입니다). |
| `-h, --help` | 도움말을 출력합니다. |
| `-V, --version` | 버전을 출력합니다. |

## 종료 코드

| 코드 | 의미 |
|---|---|
| 0 | 성공 |
| 1 | 실패 |
| 2 | 잘못된 명령줄(오타가 있으면 `leon`이 가장 가까운 명령을 제안합니다) |
| 3 | 대상을 찾을 수 없거나 모호함 |
| 4 | 서버에 연결할 수 없음 |
| 10 | `self-update --check`: 더 새로운 릴리스가 있음 |
| 130 | 중단됨(Ctrl-C) |

`leon wait`와 `leon run --wait`는 대신 프로그램 자체의 종료 상태로 종료합니다.

## 앱

### `leon app`

leon 앱을 열어 서버에 연결합니다. 이미 실행 중인 서버(CLI가 시작한 것도 괜찮습니다)이거나, `leon server start`처럼 먼저 시작한 서버입니다. CLI와 에이전트는 화면에 보이는 것과 같은 세션을 다룹니다. 명령 없는 `leon`도 같은 일을 합니다. 앱을 종료해도(⌘Q, 창 닫기) 분리될 뿐 모든 프로그램은 계속 실행되며, 앱의 "종료하고 모든 세션 끝내기(Quit and End All Sessions…)"나 `leon server stop`으로 끝냅니다. `--autostart=false`이고 서버가 없으면 4로 종료합니다.

앱이 없는 빌드(현재 Linux, 또는 `--no-default-features`로 빌드한 macOS)에는 창이 없습니다. stdin과 stdout이 터미널이면 `leon`만으로 `leon attach`가 (주어진 `-s`와 `-H`와 함께) 실행되고, 아니면 도움말을 출력하고 0으로 종료합니다(`-s`나 `-H`를 주었으면 2로 종료). `leon app`은 그 사실을 알리고 1로 종료합니다.

## 터미널에서

### `leon attach [TARGET]`

어느 터미널에서나, 로컬이든 ssh를 통해서든 leon을 씁니다(`ssh -t HOST leon attach`. `-t`가 없으면 터미널이 없으므로 1로 종료합니다). TARGET이 없으면(`-s`도 없으면) 모든 세션과 그 탭, 그 안의 블록을 보여 주는 선택기를 열고, 선택한 블록의 화면을 실시간으로 미리 보여 줍니다. 입력해서 거르고(명령 팔레트처럼 글자를 순서대로 맞춥니다), ↑와 ↓(또는 ctrl+p, ctrl+n)로 움직이고, Enter로 연결하며, Esc로 뒤로 가거나 끝냅니다. TARGET은 세션이나 블록이며, 블록이면 그 탭이 그 블록에 포커스된 채로 열립니다. 세션이 하나도 없으면 Enter로 새로 시작합니다.

연결되면 세션의 활성 탭을 보여 줍니다. 분할 레이아웃의 모든 페인이 테두리로 나뉘고, 세션과 그 탭을 보여 주는 상태 표시줄이 붙습니다. 키는 포커스된 페인으로 가지만, 접두 키와 설정의 바인딩(`init.lua`. `cmd` 키는 터미널에서 절대 오지 않습니다)은 예외입니다. 접두 키(`--prefix KEY`, 없으면 설정의 `leon.mode("attach", { prefix = … })`, 그것도 없으면 ctrl+b) 다음에:

| 키 | 하는 일 |
|---|---|
| `d` | 분리: `leon attach`는 0으로 종료하고 모든 프로그램은 계속 실행됩니다 |
| ←↑↓→ 또는 `h` `j` `k` `l` | 그 방향의 페인으로 포커스 |
| `%`, `"` | 페인을 오른쪽으로, 아래로 분할 |
| `z` | 페인 확대(다시 누르면 되돌림) |
| `c` | 새 탭 |
| `n`, `p` | 다음 탭, 이전 탭 |
| `s` | 선택기로 돌아가기 |
| 접두 키 한 번 더 | 접두 키를 페인에 입력 |

이 터미널에서의 페인 레이아웃이 바뀔 때마다(연결, 크기 조절, 분할이나 확대, 페인이 화면에 다시 나타날 때) 페인은 이 터미널에 맞는 크기가 됩니다. 그 뒤 다른 클라이언트가 크기를 바꾼 페인은 잘리거나 여백으로 채워집니다. 종료할 때(분리, 서버 정지, SIGHUP, SIGTERM, SIGINT, SIGQUIT) 터미널은 원래 상태로 돌아갑니다. SIGKILL 뒤에도 감시 프로세스가 이를 처리합니다. 시그널로 종료되면 128 + 시그널 번호로 종료합니다.

`-H`로 연결했을 때 끊긴 연결을 재시도하는 동안 상태 표시줄에 `reconnecting to HOST (try N)…`이 표시되며, 키는 페인에 전부 도착하거나 전혀 도착하지 않습니다. 호스트에서 1초 동안 아무 응답이 없으면, 입력한 키는 연결을 빠르게 확인한 뒤에 보내집니다. 확인이 실패하거나 다시 연결하는 중이면 키는 버려지고, 상태 표시줄에 보내지 못한 키의 수가 표시됩니다. 연결이 돌아오면 다시 입력하세요. 붙여넣기도 키와 함께 줄을 서서 기다립니다. 한 가지 한계가 있습니다. 그 1초 안에, 단어를 입력하는 도중에 멈춤이 시작되면 이미 보내지고 있던 키가 영향을 받아 늦게 도착할 수 있습니다.

## 세션과 창

`leon new`, `ls`, `kill`, `leon session`, `leon window`는 [cli/sessions.md](../../cli/sessions.md)에 있습니다.

## 프로그램 실행

### `leon run [FLAGS] -- CMD…`

새 창에서 명령을 실행하고 새 블록의 ID를 출력합니다. 명령은 셸을 거치지 않고 바로 실행됩니다. 파이프나 `&&`가 필요하면 `leon run -- sh -c '…'`를 쓰세요. 서버가 Finder에서 연 Leon.app으로 시작되었더라도, 명령은 로그인 셸의 PATH에서 찾습니다.

- `--label LABEL`: 블록에 이름을 붙여 `-b LABEL`로 찾을 수 있게 합니다.
- `--keep-open`: 프로그램이 종료된 뒤에도 블록을 남겨 출력을 계속 읽을 수 있게 합니다.
- `--wait`: 프로그램을 기다렸다가 그 종료 상태로 종료합니다.
- `--output[=true|false]`: `--wait`와 함께 쓰면, 프로그램이 종료된 뒤 ID 줄 다음에 프로그램의 텍스트를 출력합니다. `leon capture --trim`이 보여 줄 내용, 즉 스크롤백과 화면입니다. stdout이 터미널이 아니면(에이전트나 스크립트의 `$(…)`) 기본으로 켜지고, 터미널에서는 꺼집니다. 텍스트를 읽기 위해 `run`은 텍스트를 다 읽을 때까지 블록을 열어 두었다가 닫습니다(`--keep-open`이 아니면). 그 전에 중단된 `run --wait`는 `--keep-open`처럼 블록을 남깁니다.
- `-c, --cwd DIR`: 작업 디렉터리(기본값: 현재 디렉터리. `-H`와 함께면 호스트의 홈 디렉터리이며, `DIR`은 그 호스트의 절대 경로여야 합니다).

```sh
leon run --label tests --keep-open -- cargo test
leon run --wait -- make                       # in a script: make's output, then its status
leon run --wait --output=false -- make        # the status alone
```

### `leon split [FLAGS] [-- CMD…]`

블록을 분할하고 새 페인에서 `CMD`(기본값: 로그인 셸)를 실행하며, 새 블록의 ID를 출력합니다. `-b`(분할 자체를 가리켜 그룹 전체 옆에 페인을 둘 수도 있습니다)를 분할하고, 없으면 포커스된 블록을 분할합니다.

- `--split right|left|above|below`: 새 페인의 위치(기본값 right).
- `--ratio PCT`: 새 페인이 차지하는 비율, 1–99(기본값 50).
- `--label LABEL`, `--keep-open`, `-c, --cwd DIR`: `leon run`과 같습니다.
- `--focus[=true|false]`: 새 페인에 포커스합니다(기본값 true).

```sh
leon split
leon split --split below --ratio 30 --label logs -- tail -f /var/log/system.log
leon split --focus=false -b tests -- htop
```

### `leon send [TEXT]`

`TEXT`를 터미널에 입력합니다(기본값: stdin에서 읽기). Enter는 붙지 않으니 이어서 `leon send-key enter`를 실행하세요.

### `leon send-key KEY…`

키를 순서대로 누릅니다. 바인딩 문법으로 씁니다: `enter`, `escape`, `tab`, `space`, `backspace`, `up`, `ctrl+c`, `alt+left`, `a`…

### `leon capture`

터미널의 스크롤백과 화면을 텍스트로 출력합니다.

- `--trim`: 줄 끝 공백과 끝의 빈 줄을 지웁니다.

### `leon wait [BLOCK]`

터미널의 프로그램이 종료되기를 기다렸다가 그 종료 상태로 종료합니다(기본 블록: `-b`, 없으면 `$LEON_BLOCK`).

- `--for DURATION`: 이 시간이 지나면 포기하고(`500ms`, `10s`, `2m`) 1로 종료합니다.
- `--text TEXT`: 대신 블록의 화면이나 스크롤백에 `TEXT`가 보일 때까지 기다렸다가 0으로 종료합니다(이미 있는 텍스트도 해당됩니다). `--for`가 지나거나 블록이 먼저 닫히면 1로 종료합니다. 계속 실행되는 프로그램에 쓰입니다: 프롬프트, 개발 서버의 "ready" 줄.

```sh
id=$(leon run --keep-open -- python3)
leon wait -b "$id" --text '>>>' --for 10s    # Python's prompt: now it reads what you type
leon send -b "$id" 'print(6 * 7)'
leon send-key -b "$id" enter
leon capture -b "$id" --trim
leon send-key -b "$id" ctrl+d
leon wait -b "$id" --for 10s
```

### `leon notify`

내 블록(`-b`, 없으면 `$LEON_BLOCK`. 둘 다 없으면 2로 종료)의 상태를, 그 프로그램이 보낸 OSC 7501 보고처럼 알립니다. 탭과 페인 헤더, 앱의 활동(Activity) 목록에 표시되고, 응답 대기, 오류, 오래 걸린 실행의 완료는 알림으로도 보냅니다. OSC 7501을 쓸 수 없는 도구(훅, tty가 없는 작업)를 위한 것입니다. 블록의 루트 레코드를 바꾸며, 그 레코드의 수명을 따릅니다.

- `--state working|blocked|done|error`: 상태(기본값 `done`).
- `--title T`, `--body B`: 각각 한 줄(최대 192바이트와 2048바이트). OSC 7501이 버릴 텍스트(제어 문자, 줄바꿈, 보이지 않는 문자)는 거부되고(1로 종료) 아무것도 기록되지 않습니다.

```sh
id=$(leon run --keep-open -- sleep 30)
leon notify -b "$id" --state blocked --title "Deploy" --body "approve the rollout?"
leon block call -b "$id" program_status
leon block close "$id"
```

## 레이아웃

`leon focus`, `move`, `swap`, `zoom`, `resize`는 [cli/layout.md](../../cli/layout.md)에 있습니다.

## 블록

### `leon block`

블록(터미널 페인)을 관리합니다.

### `leon block ls`

세션의 블록을 나열합니다.

### `leon block inspect [BLOCK]`

블록을 보여 줍니다: 창, pid, 명령, 종료 상태, 그리고 프로그램이 등록했다면 재개 명령(`set_resume`).

### `leon block close [BLOCK]`

블록을 닫고 그 프로그램을 끝냅니다.

### `leon block call METHOD [ARGS]`

`size`나 `program_status` 같은 블록 메서드, 또는 `dev.leon.terminal.size` 같은 전체 이름의 메서드를 호출합니다. `ARGS`는 JSON 객체, `@file`, 또는 stdin을 뜻하는 `-`입니다.

`set_resume`은 재시작 후 이 블록의 프로그램을 재개할 명령을 등록합니다(`null`이면 지움). 복원된 블록은 설정의 `resume` 값에 따라 이를 제안합니다([lua/settings.md](../../lua/settings.md) 참고).

```sh
leon block call set_resume '{"command": ["claude", "--resume", "c7583d2c-3133-4d13-b5f0-45883f1cabcb"]}'
```

## 자동화

`leon events`, `leon do`, `leon actions`, `leon keymap`, `leon api`는 [cli/automation.md](../../cli/automation.md)에 있습니다.

## 설정과 테마

`leon config`(check, reload, get, set)와 `leon theme`(list, import, show)는 [cli/settings.md](../../cli/settings.md)에 있습니다.

## 서버

`leon server`(run, start, stop, status, proxy)는 [cli/server.md](../../cli/server.md)에 있습니다.

## 호스트

`-H HOST`는 시스템 `ssh`를 통해 다른 머신의 leon 서버에 닿습니다. `leon hosts`(list, add, remove, check. `--tailscale`은 tailnet에서 머신을 추가)와 `-H`의 동작 방식은 [cli/hosts.md](../../cli/hosts.md)에 있습니다.

## 업데이트

### `leon self-update`

leon을 최신 릴리스로 업데이트합니다. `--check`는 알리기만 합니다(10으로 종료): [updates.md](../../updates.md).

## 에이전트

`leon skill`(show, setup, status, remove), `leon float`, `leon toggle`, `leon dock`은 [cli/agents.md](../../cli/agents.md)에 있습니다.
