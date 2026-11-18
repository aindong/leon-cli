<!-- i18n: source=docs/remote-hosts.md commit=a525beb0ae17 -->
> 이 문서는 번역본입니다. 기준은 [영어 문서](../../remote-hosts.md)이며, 영어 문서가 더 최신일 수 있습니다.

# 원격 호스트

leon이 다른 머신의 leon 서버와 함께 동작하는 방식을, [README](README.md#개발-머신에서-작업하기)의 첫 단계에 이어 설명합니다. 명령은 [cli/hosts.md](../../cli/hosts.md)에 있습니다.

Wi-Fi가 끊긴 연결은 1분 동안 재시도됩니다. `leon events`와 앱은 이벤트를 잃거나 반복하지 않고 다시 이어지며, `leon attach`는 그 사실을 알린 뒤 다시 연결하고 화면을 다시 그립니다. 연결이 멈췄거나 끊긴 동안 입력한 키는 전부 도착하거나 전혀 도착하지 않으며, 상태 표시줄에 보내지 못한 키의 수가 표시됩니다(멈춤이 시작되는 순간 보낸 키는 도착할 수 있습니다). ssh 키나 ssh 에이전트를 사용하세요. 연결마다 별도의 ssh 세션입니다.

앱은 `leon hosts`의 모든 호스트를 사이드바의 이 Mac(This Mac) 아래에 보여 주며, 각 호스트에는 세션과 탭, 그리고 연결 상태를 나타내는 점이 있습니다. 원격 페인은 로컬 페인과 똑같이 동작하며 헤더에 호스트 이름이 표시됩니다. 새 세션(New Session), 새 탭(New Tab), 분할은 포커스된 페인이 있는 머신에서 동작하고, 명령 팔레트에는 머신마다 새 세션이 있으며, 호스트 추가(Add Host…)(tailnet의 머신, 또는 레이블을 입력한 뒤 `ssh://` 엔드포인트)도 있습니다. 호스트의 레이블을 클릭하면 접을 수 있습니다. 앱의 ssh는 비밀번호를 묻지 않습니다. 비밀번호가 필요한 호스트는 `needs login: run leon hosts check devbox in a terminal`이라고 표시합니다. 이를 한 번 실행하면(호스트 키를 수락하고 키 잠금을 해제) 앱이 다음 시도에서 접속합니다. 연결이 끊긴 동안 그 호스트의 페인은 마지막 화면을 유지하고 "reconnecting" 표시가 붙습니다.

![사이드바의 이 Mac(This Mac)과 devbox. 각각 세션과 탭이 있습니다](../../images/m16b-hosts.png)

![포커스된 devbox의 페인. 헤더에 호스트 이름이 있습니다](../../images/m16b-remote-pane.png)

![연결할 수 없는 호스트. 레이블 아래에 ssh가 알려 준 이유가 있습니다](../../images/m16b-unreachable.png)

![호스트 추가(Add Host…)가 tailnet의 머신을 나열하며, 각 머신의 시스템과 온라인 여부를 보여 줍니다](../../images/app-tailnet.png)
