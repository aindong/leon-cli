<!-- i18n: source=docs/install.md commit=28ab3cd4a46d -->
> 이 문서는 번역본입니다. 기준은 [영어 문서](../../install.md)이며, 영어 문서가 더 최신일 수 있습니다.

# 소스에서 leon 빌드하기

[README](README.md#설치)의 일부입니다.

leon을 직접 빌드하려면 Rust 툴체인([rustup](https://rustup.rs))과 git이 필요합니다. 아래 두 절에서 macOS와 Linux에서 빌드하는 방법을 설명합니다.

## macOS

macOS 11 이상이 필요합니다. 이 저장소를 체크아웃한 곳에서 다음을 실행합니다.

```sh
scripts/install.sh
```

이 스크립트는 `Leon.app`을 빌드해 `~/Applications`에 복사하고, `leon` 명령을 `~/.local/bin`에 링크합니다(`--prefix DIR`을 쓰면 `DIR/bin`). root 권한을 전혀 쓰지 않고, 하는 일을 하나하나 출력하며, 여러 번 실행해도 안전하고, 그 경로에 있는 것 중 자기가 설치하지 않은 것은 바꾸지 않습니다. `scripts/install.sh --uninstall`은 둘 다 제거합니다. **Leon**은 Finder, Spotlight, Dock에서 열거나 `leon`을 입력해서 실행합니다. 앱만 빌드하려면 `scripts/bundle.sh`가 `target/release/Leon.app`을 만듭니다.

앱은 leon 서버의 클라이언트입니다. 실행 중인 서버(셸에서 또는 이전에 앱을 실행할 때 시작된 것)에 연결하거나, 없으면 서버를 시작합니다. 앱을 종료해도 서버와 프로그램은 계속 실행됩니다. 이를 끝내려면 `leon server stop`을 실행하거나 leon 메뉴의 **종료하고 모든 세션 끝내기**(Quit and End All Sessions…)를 선택합니다.

## Linux

Linux에서 leon은 서버, CLI, `leon attach`(에이전트와 Lua 스크립트도)로 구성됩니다. 앱은 아직 없으므로 터미널에서 `leon`만 실행하면 `leon attach`가 실행됩니다. `cargo`로 빌드합니다.

```sh
cargo build --release -p leon
```

그러면 헤드리스 `target/release/leon`이 만들어지니 `PATH`에 두세요. glibc 2.34 이상이 필요합니다(Debian 12, Ubuntu 22.04, RHEL 9 이상). 세션이 `$XDG_RUNTIME_DIR`을 설정하면 소켓은 `$XDG_RUNTIME_DIR/leon`에 놓입니다. Docker가 있으면 대신 패키지를 빌드할 수 있습니다.

```sh
scripts/package-linux.sh                 # or --arch amd64 / --arch arm64
```

이 스크립트는 tarball과 `.deb`를 `target/linux/<arch>/`에 쓰고, 둘 다 깨끗한 컨테이너에서 설치되고 실행되는지 확인합니다. Debian이나 Ubuntu에서는 `.deb`를 `dpkg`로(root로) 설치합니다.

```sh
dpkg -i target/linux/$(dpkg --print-architecture)/leon_*.deb
```

또는 tarball을 아무 곳에나 풀고 그 안의 `leon`을 `PATH`에 둡니다.

```sh
tar xzf target/linux/*/leon-*-"$(uname -m)"-linux.tar.gz -C ~/.local
ln -s ~/.local/leon-*-linux/leon ~/.local/bin/leon
```

## 릴리스 파일

`scripts/release.sh`는 커밋 하나의 깨끗한 클론에서 릴리스에 들어가는 파일을 빌드합니다. `Leon-<version>-macos-<arch>.zip`(Leon.app, ad hoc 서명, 공증 없음), macOS용 CLI tarball, Linux arm64와 x86_64용 tarball과 `.deb`, 그리고 `SHA256SUMS`입니다. macOS에서 실행되며 Linux 패키지에는 Docker가 필요합니다. `leon --version`은 릴리스와 빌드한 커밋을 보여 줍니다. 태그를 push하면 GitHub에서 같은 작업을 하고, 모든 파일에 서명한 뒤 릴리스를 게시합니다: [releases.md](../../releases.md).
