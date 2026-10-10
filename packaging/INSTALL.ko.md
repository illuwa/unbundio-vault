# unbundio-vault 설치 (일반 사용자용, 무료)

Rust·터미널 지식 없이 설치할 수 있습니다. 계정·결제 없음, 평생 무료.

## 1. 다운로드

[Releases](https://github.com/illuwa/unbundio-vault/releases) 에서
`unbundio-vault-0.1.0-macos-universal.zip` 을 받아 압축을 풉니다.

## 2. 설치 스크립트 실행

```sh
cd unbundio-vault-0.1.0-macos-universal
./install.sh
```

> **"확인되지 않은 개발자" 경고가 뜨면** (서명 없는 무료 배포라 정상):
> Finder에서 `unbundio-vault` 우클릭 → **열기** → 다시 **열기** 클릭.
> 1회만 하면 이후 터미널·브라우저에서 정상 실행됩니다.
> 터미널 선호 시: `xattr -d com.apple.quarantine unbundio-vault`

마스터 비밀번호를 두 번 입력하면 볼트가 만들어집니다
(`~/unbundio-vault.vault`). 화면에 표시되지 않는 게 정상입니다.

## 3. Chrome 확장 로드 (1회)

1. `chrome://extensions` → 우상단 **개발자 모드** 켜기
2. **압축해제된 확장 프로그램을 로드합니다** → 압축 푼 폴더 안의 `extension` 폴더 선택
3. 등록된 확장 ID(알파벳 32자) 복사

## 4. 브라우저 연결 (1회)

```sh
./install.sh --extension-id <복사한-ID>
```

호스트 비번 저장 여부를 묻습니다(기본 No). Yes면 브라우저 자동완성이
별도 입력 없이 동작하고, No면 환경변수로만 잠금해제됩니다.

## 5. 사용

로그인 페이지에서 툴바 아이콘 → 해당 항목 **Fill**.
비밀번호만 필요하면 **Copy**.

## 삭제

```sh
rm ~/.local/bin/unbundio-vault ~/unbundio-vault.vault
rm ~/Library/Application\ Support/Google/Chrome/NativeMessagingHosts/com.unbundio.vault.json
rm -rf ~/.config/unbundio-vault   # 호스트 비번을 저장했을 경우
```

## Dashlane/1Password에서 이사 오기

기존 앱에서 CSV 내보내기 → (옮긴 뒤 파일 삭제):

```sh
unbundio-vault import --path 내보낸.csv
unbundio-vault audit
```

## 무료 약속

- MIT 라이선스, 소스 전부 공개. 요금제·계정·클라우드 없음.
- 팀 공유가 필요하면 같은 집의 `keep-my-password`를 쓰세요
  (이 도구는 영원히 개인·로컬용입니다).
