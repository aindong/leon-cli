<!-- i18n: source=docs/recipes.md commit=28ab3cd4a46d -->
> 이 문서는 번역본입니다. 기준은 [영어 문서](../../recipes.md)이며, 영어 문서가 더 최신일 수 있습니다.

# 프로젝트와 git worktree

[examples/project.lua](../../../examples/project.lua)는 `project_open`이라는 Lua 액션을 정의합니다. [examples/projects.lua](../../../examples/projects.lua)는 같은 레시피를 `leon do`로 실행합니다. 둘 다 레이블이 프로젝트 폴더 이름과 같은 세션을 찾고, 없으면 첫 터미널이 그 디렉터리에서 열리는 세션을 만듭니다. 생성에는 `session.create`의 `cwd`를 쓰며, 이는 `leon new --cwd DIR`과 같은 매개변수입니다. 프로젝트와 worktree에 관한 정책은 이 예제 안에만 있으니 복사해서 원하는 대로 고치세요.

이 예제들은 로컬에서 실행됩니다. 로컬 서버와 로컬 폴더를 사용하세요. 스크립트 경로는 호출한 쪽의 디렉터리를 기준으로 해석합니다. 액션은 서버 안에서 실행되므로 `dir`에 절대 경로가 필요합니다. 세션 이름은 전역입니다. 폴더 이름과 같은 세션이 이미 있으면, 그 셸이 이후에 디렉터리를 옮겼더라도 그 세션을 다시 씁니다. 세션 레이블이 중복되거나 폴더 이름이 같은 worktree가 있으면 오류가 나니 먼저 이름을 바꾸세요. 폴더 이름은 leon의 레이블 조건(128자 이하, 제어 문자 없음)에 맞아야 합니다.

## 프로젝트 하나

leon 체크아웃에서 `leon`이 PATH에 있는 상태로 다음을 실행합니다.

```sh
leon do ./examples/projects.lua dir="$PWD"
```

스크립트는 세션 ID를 출력합니다. `leon attach SESSION_ID`로 터미널에서 열거나, 앱에서 그 세션을 선택하세요. 스크립트를 다시 실행해도 같은 ID가 출력되고 터미널이 더 생기지 않습니다. 다른 프로젝트는 `dir=/path/to/project`나 `dir=../project`로 넘길 수 있습니다. 레시피 없이 새 세션을 만들려면 `leon new project-name --cwd /path/to/project`를 사용합니다.

## 모든 worktree

```sh
leon do ./examples/projects.lua dir="$PWD" worktrees=true
```

레시피는 `git worktree list --porcelain -z`를 읽고, 메인 체크아웃을 포함해 쓸 수 있는 worktree마다 세션 ID를 하나씩 출력합니다. NUL 구분자 덕분에 상위 경로의 공백, 따옴표, 줄바꿈이 그대로 보존됩니다. bare 저장소와 prunable로 표시된 worktree는 건너뜁니다. worktree를 만들거나 지우지는 않으니 `git worktree add`와 `git worktree remove`는 직접 실행하세요.

## 무시된 파일을 새 worktree로 복사하기

worktree를 만든 뒤, 그 세션이 만들어지기 전에 지정한 무시된 파일을 복사하도록 선택할 수 있습니다. 여기서 `.env`는 원본 루트에 있는, 무시된 일반 파일이어야 합니다.

```sh
leon do ./examples/projects.lua dir="$PWD" worktrees=true \
  copy_from="$PWD" 'copy_files=[".env"]'
```

이 훅은 새로 만들어지는 세션에서만 실행됩니다. 대상 위치에 이미 있는 파일은 건드리지 않으며, 새 복사본은 비공개입니다(모드 0600, 파일 시스템에 따라 다름). 이름은 루트의 파일 이름이어야 하며 `/`나 `..`를 쓸 수 없습니다. 심볼릭 링크와 Git이 무시된 파일로 보지 않는 파일은 거부됩니다. 기본적으로는 무시된 파일을 하나도 복사하지 않습니다. `copy_from`은 원본 체크아웃이고, `copy_files`는 이름의 JSON 배열입니다. 훅이 실패하면 레시피는 그 세션을 만들기 전에 멈춥니다. 이미 만든 세션과 복사본은 남습니다. 레시피는 한 번에 하나만 실행하세요. 세션 목록 조회와 생성은 별개의 API 호출입니다.

## 액션 설치하기

`examples/project.lua`를 `init.lua` 옆에 복사한 뒤, 다음 설정을 추가합니다(바인딩한 폴더는 내 프로젝트의 절대 경로로 바꾸세요).

```lua
require("project")
leon.bind("cmd+shift+o", "project_open", { dir = "/path/to/project" })
```

`leon config reload`로 다시 불러옵니다. `leon do project_open dir=/path/to/project`는 세션 ID의 배열을 반환합니다. `leon attach SESSION_ID`나 앱에서 하나를 선택하세요. 이 바인딩은 세션을 만들거나 찾기만 하고, 어느 클라이언트에서 선택할지는 사용자에게 맡깁니다. 액션은 스크립트와 같은 `worktrees`, `copy_from`, `copy_files` 인수를 받습니다.

모듈은 준비 훅으로 `before_create(dir, args)`를 공개합니다. `require` 뒤에 이를 바꾸거나, 기본 동작(무시된 파일 복사)을 유지하려면 감싸세요. 준비에 실패하면 훅은 `error(message)`로 실패해야 합니다. 훅은 `session.create` 전에 실행되며, 액션에는 leon의 일반적인 30초 기한이 있습니다.

이 문서의 모든 `sh`와 `lua` 블록은 `m21_recipe_docs`에서 실행됩니다. `m21_recipe`는 git worktree 두 개와 무시된 파일 복사를 포함해, 실제 스크립트와 액션을 비공개 서버에서 확인합니다.
