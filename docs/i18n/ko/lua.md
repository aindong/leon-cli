<!-- i18n: source=docs/lua.md commit=a525beb0ae17 -->
> 이 문서는 번역본입니다. 기준은 [영어 문서](../../lua.md)이며, 영어 문서가 더 최신일 수 있습니다.

# Lua: 설정과 스크립트

leon은 두 곳에서 Lua 5.1을 사용합니다.

- **설정**, 즉 `init.lua`. 서버가 시작할 때 불러옵니다: 키 바인딩, 모드, 직접 만든 액션. `leon config check`로 확인하고 `leon config reload`로 변경 사항을 적용합니다.
- **스크립트**. `leon do`가 내 머신에서 서버를 상대로 실행합니다: 일회성 자동화, 또는 이벤트에 반응하며 오래 실행되는 감시 스크립트.

둘 다 `leon.*` 함수를 공유합니다([목록](../../lua/scripts.md#function-list)). 이 문서들의 모든 예제는 테스트(`crates/leon/tests/m7b_lua_doc.rs`)가 실행합니다.

## 설정 파일 위치

서버는 다음 중 처음 찾은 것을 읽습니다: `server run --config PATH`, `$LEON_CONFIG`, `$XDG_CONFIG_HOME/leon/init.lua`, `~/.config/leon/init.lua`. 파일이 없으면 빈 설정으로 봅니다. `require`는 그 옆의 파일을 불러옵니다.

```sh
leon config check
leon config reload
leon keymap
leon actions
```

설정은 불러올 때마다 위에서 아래로 실행됩니다. 잘못된 `leon.*` 호출은 `file:line` 메시지와 함께 건너뛰고 나머지는 계속 불러옵니다. 문법 오류는 불러오기 전체를 실패시키며, 서버는 마지막으로 올바르던 설정을 유지합니다. 두 경우 모두 `leon config check`는 1로 종료합니다.

## 문서 목록

- [키와 모드](../../lua/keys.md): `leon.bind`, 시퀀스, 반복 키, tmux 스타일 프리셋, vim-navigator, 모드와 `leon attach`의 접두 키, 함수 바인딩, 플로팅 페인, 스크래치 셸 키.
- [직접 만든 액션](../../lua/actions.md): `leon.action`, `leon.session.<method>`, `leon.call`.
- [설정 항목](../../lua/settings.md): 모든 `leon.set` 설정과 그 값, 기본값.
- [스크립트와 함수](../../lua/scripts.md): `leon do`, 이벤트, 타이머, `leon.exec`, `leon.*` 함수 목록과 제한.

## 프로젝트 세션

폴더마다 세션을 재사용하고, 선택적으로 worktree 세션과 무시된 파일 준비 훅을 쓰는 예제 액션과 `leon do` 스크립트는 [프로젝트와 git worktree](recipes.md)를 참고하세요.
