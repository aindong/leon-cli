<!-- i18n: source=docs/agents.md commit=28ab3cd4a46d -->
> 이 문서는 번역본입니다. 기준은 [영어 문서](../../agents.md)이며, 영어 문서가 더 최신일 수 있습니다.

# 에이전트가 leon을 다룹니다

leon 터미널에서 실행되는 코딩 에이전트(Claude Code, Codex)는 사람이 거기서 하는 일을 똑같이 할 수 있습니다. 자기 옆에 페인을 열고, 그 안에서 테스트를 실행하고, 끝나기를 기다리고, 화면을 읽고, REPL에 입력하고, 자기가 연 것을 닫습니다. 사용하는 것도 사람이 쓰는 것과 같은 `leon` 명령입니다. 이 문서는 에이전트에게 이를 가르치는 방법과, 배운 에이전트가 어떻게 행동해야 하는지를 설명합니다.

## 한 번만 설정하기

```sh
leon skill setup --dry-run   # show what it would write
leon skill setup             # install for every agent it finds
leon skill status            # current, outdated, missing, foreign or unreadable
```

`leon skill setup`은 찾은 에이전트마다 두 가지를 설치합니다. 설정 디렉터리가 있거나 프로그램(`claude`, `codex`)이 PATH에 있는 에이전트가 대상이며, 한 번도 실행한 적이 없어도 대상이 됩니다. 에이전트마다 선택한 이유와 기록한 파일을 하나하나 출력합니다. 둘 다 찾지 못하면 공유 디렉터리 `~/.agents/skills/leon/`(다른 에이전트도 이것을 읽습니다)만 기록하고 그 사실을 알려 줍니다. `--agent claude|codex`로 하나만 고를 수 있습니다.

| | Claude Code | Codex |
|---|---|---|
| 스킬: `SKILL.md` 스텁 | `~/.claude/skills/leon/`(`$CLAUDE_CONFIG_DIR`) | `~/.agents/skills/leon/` |
| `leon skill hook --agent …`를 실행하는 세션 시작 훅 | `~/.claude/settings.json` | `~/.codex/hooks.json`(`$CODEX_HOME`) |

- **스킬**은 에이전트에게 `leon skill show`를 실행하라고 알려 줍니다. 이 명령은 `leon` 바이너리에 컴파일되어 들어 있는 전체 가이드를 출력합니다. leon을 업그레이드하면 가이드도 업그레이드되며, 다시 설치할 것은 없습니다.
- **훅**은 에이전트 세션이 시작될 때 실행됩니다. leon 안에서는 에이전트에게 자신의 세션과 터미널 블록을 알려 주고, 그 세션을 재개하는 방법을 기록합니다(아래 참고). leon 밖에서는 아무것도 출력하지 않고 서버에 연결하지도 않습니다. 이전 버전 leon의 훅은 `outdated`로 표시되니 `leon skill setup`을 실행하세요.

설치는 여러 번 실행해도 안전합니다. 설정 파일을 그 자리에서 고치며 기존 키와 그 순서를 유지하고, 파일을 한 번만 `<file>.leon-backup`으로 백업하며, 예상한 JSON 구조가 아닌 파일은 건드리지 않습니다. 자기가 기록하지 않은 `SKILL.md`는 `--force`를 주지 않는 한 바꾸지 않습니다(이전 파일은 `SKILL.md.leon-backup`으로 남습니다).

```sh
leon skill show                    # read the guide the agent reads
leon skill remove --dry-run
leon skill remove                  # removes what setup installed, and nothing else
```

## 나는 어디에 있나: `LEON_SESSION`과 `LEON_BLOCK`

모든 leon 터미널에는 변수 두 개가 있습니다.

- `LEON_SESSION`: 세션의 ID. 예: `session:3f2a…`.
- `LEON_BLOCK`: 이 터미널 자체의 ID. 예: `block:91c0…`.

leon 명령은 이것들을 기본값으로 쓰므로, 에이전트의 `leon run`은 아무 플래그 없이도 *자기* 세션에, 사용자의 다른 작업 옆에 탭을 엽니다. 자기 세션이란 지금 `LEON_BLOCK`을 담고 있는 세션입니다. 다른 세션으로 옮겨진 페인(앱의 전환기에서 ⌘↩)은 시작할 때의 `LEON_SESSION`을 그대로 가지고 있습니다. 실행 중인 프로그램의 환경은 바꿀 수 없기 때문입니다. 그래서 leon은 블록을 찾아보고, `LEON_BLOCK`이 설정되지 않았거나 이미 사라졌을 때만 `LEON_SESSION`을 씁니다. 두 변수 모두 없으면 에이전트는 leon 안에 있지 않은 것입니다. 이때는 `leon ls`를 실행하고 어느 세션을 쓸지 물어봐야 합니다.

에이전트는 사용자의 말을 다음과 같이 해석해야 합니다.

- "여기", "네 옆에", "이 세션에서": 자기 세션, 즉 `LEON_BLOCK`을 담고 있는 세션.
- "테스트 페인", "api 세션": 사용자가 말한 이름이 항상 기본값보다 우선합니다. `-b`나 `-s`로 넘기세요.
- 이름이 여러 대상과 일치하면 leon은 그 목록을 보여 주고 3으로 종료합니다. 추측하지 말고 물어보세요.

**함정 하나:** `-b` 없는 `send`, `send-key`, `capture`는 `LEON_BLOCK`, 즉 에이전트 자신의 터미널에 적용됩니다. 거기에 입력하면 에이전트 자신의 입력창에 입력하게 됩니다. 의도한 블록을 항상 `-b`로 넘기세요.

## 에이전트 쪽에서 본 세션

```sh
# open a pane beside me, run the tests there, keep it after they finish
id=$(leon split --label tests --keep-open -- cargo test)
leon wait "$id"; echo "tests exited $?"
leon capture -b "$id" --trim | tail -40

# drive a REPL
py=$(leon run --label py -- python3 -q)       # -q: no banner to match
leon wait -b "$py" --text '>>>' --for 10s     # its prompt, before typing
leon send -b "$py" 'print(6 * 7)'
leon send-key -b "$py" enter
leon wait -b "$py" --text 42 --for 10s
leon capture -b "$py" --trim | tail -3

# clean up what I created, and only that
leon block close "$id"
leon block close "$py"
```

가이드가 에이전트에게 주는 규칙입니다.

- 만드는 블록마다 레이블을 붙여서, 사용자가 자기 것과 구별할 수 있게 합니다.
- 일이 끝나면 자기가 만든 것을 닫습니다. 사용자의 블록이나 자기 자신의 `LEON_BLOCK`은 절대 닫지 않습니다.
- 요청받지 않는 한 사용자의 터미널을 읽거나 거기에 입력하지 않습니다. 화면에는 비밀 정보가 있고, 보낸 키는 사용자의 셸에서 실행됩니다.
- 계속 실행되는 프로그램(개발 서버, REPL)은 종료를 기다리지 말고, 기대하는 텍스트를 기다립니다: `leon wait -b ID --text TEXT --for 30s`.
- 에이전트가 `leon run --wait -- CMD`를 실행하면 한 번의 호출로 프로그램의 출력을 보여 주고 그 종료 상태로 종료합니다.
- 파싱할 출력에는 `--json`을 쓰고, 명령으로 다루지 못하는 것은 `leon api list` / `leon api describe`로 해결합니다.

## 재시작 후에도 에이전트가 돌아옵니다

서버가 멈추면(`leon server stop`, 로그아웃, 재부팅) 각 블록의 텍스트가 저장되고, 복원된 블록은 흐린 `— restored from <time> —` 줄과 새 셸 위에 그 텍스트를 보여 줍니다. 프로그램은 살아남지 않으며, 저장된 명령이 다시 실행되는 일도 없습니다. 돌아오는 것은 프로그램이 `set_resume`으로 자기 블록에 등록한 명령입니다. 훅이 이를 대신 해 줍니다. Claude Code는 `claude --resume <id>`, Codex는 `codex resume <id>`를, 에이전트가 훅에 넘겨주는 세션 id로 등록합니다. 터미널의 포그라운드에 있는 에이전트에 대해서만 기록하므로, 다른 에이전트의 도구가 중첩으로 실행한 `claude -p`가 대화형 세션의 명령을 덮어쓰지 않습니다. Claude Code의 출력 모드(`-p`, `--print`)에 대해서도 기록하지 않습니다. 일회성 실행에는 재개할 것이 없기 때문입니다.

다른 프로그램도 자기 명령을 등록할 수 있습니다.

```sh
leon block call set_resume -b "$LEON_BLOCK" '{"command": ["my-agent", "--resume", "42"]}'
leon block inspect "$LEON_BLOCK"        # shows "resume: my-agent --resume 42"
leon block call set_resume -b "$LEON_BLOCK" '{"command": null}'
```

복원될 때 블록은 `init.lua`의 `resume` 설정(`leon.set("resume", "ask")`)을 따릅니다. `"ask"`(기본값)는 새 프롬프트에 명령을 입력해 두고 Enter를 기다리며(Ctrl-C로 건너뜀), `"always"`는 바로 실행하고, `"never"`는 무시합니다. 명령은 다음 재시작 한 번에만 제안됩니다. 한 번 입력되면 다시 저장되지 않으므로, 건너뛰었거나 오래된 명령이 매번 돌아오지 않습니다. 재개된 에이전트의 훅이 다시 등록합니다.

## 프로그램 상태: 에이전트가 무엇을 하는지 사용자에게 보여 주기

에이전트와 도구는 OSC 7501로 자기 상태(`working`, `blocked`, `done`, `error`)를 알릴 수 있고, leon은 이를 터미널마다 추적합니다. `leon block call program_status -b "$LEON_BLOCK"`으로 다시 읽을 수 있으며, 변경될 때마다 `leon events -s SESSION`에 `program_status_changed` 이벤트가 나옵니다.

### OSC 7501 형식

보고 하나는 터미널에 쓰는 이스케이프 시퀀스 하나입니다. `ESC ] 7501 ;`, 다음으로 `:`로 이은 `key=value` 쌍, 마지막으로 `BEL`(또는 `ESC \`)입니다. 키는 영문 소문자입니다. 값에는 영문자, 숫자, `_ . , + / = -`만 쓸 수 있습니다. `id`는 `/`로 이은 최대 8개의 세그먼트이며, 각 세그먼트는 영문자, 숫자, `_ . + -`로 된 1~32자입니다. `ESC ] 7501 ; ? BEL`을 보내면 터미널이 무엇을 지원하는지 물을 수 있고, leon은 `ESC ] 7501 ; ? : states=idle,working,done,blocked,error : kinds=permission,question,auth ESC \`(공백 없이)로 답합니다.

예를 들어 `printf '\033]7501;state=working:progress=40\007'`은 프로그램이 40% 진행되었다는 뜻입니다.

- **키.** `state`(필수): `idle`(알릴 것 없음), `working`, `done`, `blocked`, `error`, `clear` 중 하나. `id`는 레코드의 이름입니다(없으면 루트 레코드, `/`로 자식을 만듭니다). `kind`(`blocked`와 함께일 때만): `permission`, `question`, `auth`. `progress`: 0~100, `working`이나 `blocked`와 함께 씁니다(없으면 알 수 없음). `app`: `[A-Za-z0-9_.+-]{1,32}`, 이를 설정한 가장 가까운 조상에게서 가져옵니다. `title`과 `msg`: 제어 문자가 없는 UTF-8 한 줄을 base64로 인코딩한 것.
- **규칙.** 공백은 잘라 내고, 형식이 잘못된 쌍은 건너뛰며, 모르는 키는 무시하고, 중복된 키는 마지막 것이 적용됩니다. 모르는 `state`나 잘못된 `id`는 보고 전체를 버립니다(잘못된 `id`가 루트로 대체되는 일은 없습니다). 각 보고는 자기 레코드를 통째로 바꿉니다. `clear`는 레코드와 그 자식을 지웁니다(`id`가 없으면 모든 레코드).
- **수명.** `working`과 `blocked`는 프로세스가 종료되거나 새 프롬프트가 시작되면(OSC 133 `A`) 사라집니다. `done`과 `error`는 그 페인을 볼 때까지 남습니다. 전체 리셋(`RIS`)은 레코드를 지우지만, 소프트 리셋과 대체 화면 전환은 지우지 않습니다.
- **한도.** 다음 중 하나라도 넘으면 보고 전체가 버려집니다: 시퀀스 4096바이트, 키 16, `msg` 인코딩 후 2732(디코딩 후 2048), `title` 256(192), `app` 32, `id` 전체 128에 세그먼트당 32, 깊이 8. 터미널 하나는 레코드를 256개까지 보관하며, 가장 오래 쓰이지 않은 것부터 버립니다.
