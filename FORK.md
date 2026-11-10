# Hex Haas fork

日本語音声認識モデル Haas-ja-v1.0 をWindows版Hexから使うための個人用Forkです。公式Hexに代えて普段使いする前提です。公式Hexのリリースではありません。

- Fork: https://github.com/ymkn/hex-haas
- 上流: https://github.com/anomalyco/hex
- 改修ブランチ: `haas`
- ベース: 公式 `v3.3.0`（`v3.0.0` `7b05b5fd1fa0e3b9c333209cd1644d9603693d5c` に `v3.1.1`、`v3.3.0` を順にマージ）、さらに 2026-11-08 に `upstream/dev`（`bc996e85a83fa6310ee646fb54aa23c0214ce67a`）をマージ
- `dev` は上流用として残し、個人の変更は `haas` にだけコミットします。

Haasは speech.cpp v0.10.0 の常駐worker経由で動きます。Whisperなど既存モデルは元の認識処理を使います。15秒以内の音声は分割せず、Haas自身の句読点出力を保持します。詳細・制約・検証結果は [docs/haas-ja.md](docs/haas-ja.md) を参照してください。

Haas対応以外の使い方は通常のHexと同じで、ログイン時起動も通常どおり使えます。保存先のディレクトリ名だけを `hex` から `hex-haas` に変えています。

- 設定・履歴: `%APPDATA%\hex-haas`
- モデル・ログ: `%LOCALAPPDATA%\hex-haas`（上流のWindows配置に従う。上流 v3.3.0 時点の `%APPDATA%` 配置から移動済み）
- `hex.config.ts`: `~/.config/hex-haas`

`dist/haas/hex.exe` を直接起動できます。`Start-Hex-Haas.cmd`（`scripts/start-haas.ps1`）は、ビルド済みの版を GPU → x64 CPU → ARM64 CPU の順で探して起動します。専用の環境変数や起動先切り替えは不要です。ローカルビルドのため、公式のローカルビルドと同じく自動更新は行いません。

## ローカル作業場所とリモート

clone先はどこでも構いません（例: `~/Desktop/hex-haas`）。Gitのリモートは次の役割に分けます。

```text
origin   -> ymkn/hex-haas     自分の変更の取得・push
upstream -> anomalyco/hex     公式更新の取得のみ
```

`remote.pushDefault=origin`、`pull.ff=only` を設定し、`upstream` へのpush URLは意図的に無効化しています。これらのGit設定はclone先へ自動継承されません。別PCでは以下で構成します。

```powershell
git clone --branch haas https://github.com/ymkn/hex-haas.git hex-haas
Set-Location hex-haas
git remote add upstream https://github.com/anomalyco/hex.git
git remote set-url --push upstream https://example.invalid/upstream-push-disabled
git config remote.pushDefault origin
git config pull.ff only
```

### 自動化の identity（ymkn-muse）

Muse による自動作業（上流同期のコンフリクト解消、Issue の対応、検証ビルドの起動など）は、専用の機械アカウント **ymkn-muse** の名義で行います。このリポジトリのコラボレーター（書き込み可）として登録されています。

- cube の作業 clone（`~/muse/hex-haas-push`）は author を ymkn-muse に設定し、origin は `https://ymkn-muse@github.com/ymkn/hex-haas.git` を使います。認証は ymkn-muse の classic PAT（`repo`＋`workflow`、名前 `muse-cube-hex-haas`）で、cube の `~/.git-credentials-muse` にのみ保管します（ymkn 本人の認証情報とは分離）。
- fine-grained PAT は他人の個人リポジトリ（コラボレーター参加）には使えないため、classic PAT を使います。権限の実効範囲は ymkn-muse が参加しているリポジトリに限られます。

## 上流の変更を取り込む

ローカルビルドの更新は手動です。公式更新の取得、ソースへのマージ、ビルド、実行ファイルの入れ替えは別々に行います。公式をfetchしただけでは、使用中のHexは変わりません。

安定版を使い続ける場合は、まず上流のリリースタグを確認し、取り込むタグを選びます。`upstream/dev` は開発中の変更を含むため、安定版より競合・不具合の可能性があります。

```powershell
git switch haas
git status --short  # 未コミット変更があれば先に保存する
git fetch upstream --tags
git tag --list 'v*' --sort=-version:refname

# 選んだ安定版タグを代入する。例示文字列のまま実行しない。
$target = '選んだ公式リリースタグ'
git log --oneline HEAD..$target
git diff --stat HEAD...$target
git merge --no-ff $target

# 開発版を意図して取り込む場合は、タグの代わりに upstream/dev を指定する。
```

競合したら、`git status` に表示されるファイルで公式の変更とHaas対応を両立させ、対象ファイルを `git add` して `git merge --continue` します。マージ前がcleanで、取り込み自体を中止する場合のみ `git merge --abort` を使います。`reset --hard` やforce-pushは通常の更新に必要ありません。

特に `transcription/models`、`transcription/local_transcriber`、`labs`、`app_paths`、`Cargo.toml` の変更を確認してください。上流のファイル移動やAPI変更には手動適応が必要です（v3.1.1では `experiments` が廃止され `labs` になり、`transcription_models` や `windows_login_item` はディレクトリーへ移動しました）。公式のRune/native依存ピンは、単に「最新」という理由で個別に変えず、選んだ上流との互換性を検証します。

```powershell
./scripts/build-haas.ps1 -Check
./scripts/build-haas.ps1 -Lint
./scripts/build-haas.ps1 -TestFilter haas
./scripts/build-haas.ps1 -BuildOnly

# この後、起動中のHexをトレイから終了する。
Copy-Item ./target/release/hex.exe ./dist/haas/hex.exe -Force
./scripts/test-haas-package.ps1
uv run --no-project python ./scripts/check_haas_punctuation.py  # Pythonのみで動く。uvがなければ python でもよい
git push origin haas
```

公開音声や合成音声によるテストと、実マイク・ショートカット・貼り付け確認は区別してください。既存の全workspaceテストにはWindows非対応の `/bin/sh` 前提テストが1件あり、詳細は検証記録に記載しています。

## 再構築とデータ

必要なものは Visual Studio x64 C++ツール・Windows SDK・Rust・PowerShell 7・Git（`-TestFilter` のbash用）です。推論にはVulkan対応GPUドライバー（`vulkan-1.dll`）も必要です。`setup-haas-build-tools.ps1` がCMake・shaderc・Vulkan/SPIRVヘッダー・speech.cpp v0.10.0を固定バージョンで取得し、ハッシュ検証してから展開します。

```powershell
./scripts/setup-haas-build-tools.ps1
./scripts/build-haas.ps1
./Start-Hex-Haas.cmd
```

Haasモデルはモデル選択から取得します。モデルの推奨や初回セットアップの動作は通常のHexと同じです。ランチャーは起動するだけで、設定のコピーやHaasへの強制変更は行いません。設定とモデルはGitからは復元されません。

以前の `haas-data/` と `haas-config/` を引き継ぐ場合は、Hexを終了し、それぞれ `%APPDATA%\hex-haas` と `~/.config/hex-haas` に移します。移動先に既存データがある場合は上書きしないでください。通常版の `%APPDATA%\hex` と `~/.config/hex` は読み書きしません。

GitHubへ保存するのはソース・スクリプト・文書だけです。次は `.gitignore` で除外しています。

- `haas-data/`、`haas-config/`: 旧保存先（移行前の個人データを誤ってコミットしないため引き続き除外）
- `tools/haas/`: ダウンロードしたツール、検証音声、旧バイナリのバックアップ
- `target/`、`dist/`: ビルド結果と実行ファイル

これらの除外フォルダーは必要に応じて別途バックアップしてください。Forkは上流のライセンスを保持します。上流への提案は、外部PRを受け付けない公式の方針に従ってIssueを使います。

## ビルド済みパッケージを取得する

Releases に `Hex-Haas-<version>-windows-x64.zip`（GPU版）、
`Hex-Haas-<version>-windows-x64-cpu.zip`（CPU版）、
`Hex-Haas-<version>-windows-arm64-cpu.zip`（ARM64 CPU版）を置いています。
ソースをビルドせずに使えます。

- 展開して `hex.exe` を実行するだけです。`speech.exe` と `speech.dll` は同じフォルダーに置きます。
- 必要なものは Windows 10/11 x64 と Vulkan対応GPUドライバーのみ。Haasモデルは初回起動後にモデル選択からダウンロードします（huggingface.co）。
- コード署名はありません。`SHA256SUMS` でハッシュを確認し、SmartScreenの警告は「詳細情報」から実行を続けてください。
- データは `%APPDATA%\hex-haas`（設定・履歴）と `%LOCALAPPDATA%\hex-haas`（モデル・ログ）に作られ、公式Hexとは混ざりません。更新は配置先フォルダーで `hex haas-upgrade` を実行して手動で行います（後述）。

リリースの作り方（自分用）: `hex-v<バージョン>` 形式のタグをpushすると Actions の「Haas release」がGPU版・x64 CPU版・ARM64 CPU版をビルドして、3つのzipと統合したSHA256SUMS、それに `manifest.json`（バリアント別のファイル名・SHA-256・サイズ）を1つのリリースに登録します。タグなしの `workflow_dispatch` は同じビルドを検証し、成果物をworkflowのartifactとして残すだけです。ローカルでも `./scripts/build-haas.ps1` の後に `./scripts/package-haas.ps1`、CPU版は `./scripts/build-haas-cpu.ps1`（ARM64は `-Arch arm64`）の後に `./scripts/package-haas-cpu.ps1` で同じzipを作れます。

バージョン番号の付け方: フォーク側で勝手に上げることはせず、`Cargo.toml` の version は「本家（上流）のバージョン + `-haas.<連番>`」とします（例: 本家 3.3.0 ベースなら `3.3.0-haas.1`、その次のリリースは `3.3.0-haas.2`）。ここでいう本家のバージョンは、取り込み時点で上流の `Cargo.toml` に書かれている version そのものです。その値が変わったら連番は 1 から振り直します。タグは `hex-v<そのバージョン>`（例: `hex-v3.3.0-haas.1`）です。`hex haas-upgrade` の数値比較は `-` 区切りの連番も数値として拾うため、連番同士の新旧判定がそのまま効きます。なお、この方式の前にフォーク独自に付けた 3.3.1 までの導入済みパッケージは、アップデーター自体を持たないため、初回だけ手動で入れ替えてください（3.3.2 はリリース公開前に取り下げたため、導入済みのものは存在しません）。

## デイリービルドと端末側アップデート

- Actions の「Haas daily sync and build」が毎日03:30（JST）に上流 `anomalyco/hex` の `dev` を `haas` にマージします。きれいにマージできればそのまま push し、3バリアントを再ビルドして prerelease `haas-daily` を差し替えます。コンフリクトしたらマージを取りやめ（`haas` は変更しません）、`haas-sync-conflict` ラベルの Issue で知らせます。上流にもフォークにも前回のデイリーから差分がない日はビルドしません。手動 dispatch の `force` で強制実行できます。
- 各パッケージの zip には `haas-build.json`（variant / channel / version / build）が入り、各リリースには `manifest.json` が付きます。
- 導入済み端末での更新は、Hex を終了してから配置フォルダーで `hex haas-upgrade` を実行します。自分のチャンネルの最新 `manifest.json` と比べて新しければ、同じバリアントの zip をダウンロードし、サイズと SHA-256 を検証してからファイルを入れ替えます。`--check` は確認のみ、`--yes` は確認プロンプト省略、`--channel daily|stable` はチャンネルの一時上書きです。
- チャンネルはパッケージ側で決まります。タグ版は stable（最新の `hex-v*` リリースを追う）、デイリー版は daily（`haas-daily` を追う）。
- `haas-build.json` がない旧パッケージ（v3.3.1まで）は、初回だけ `hex haas-upgrade --variant gpu`（CPU版は `--variant cpu`、ARM64版は `--variant arm64`）とバリアントを指定してください。以後はマーカーで自動判定されます。
- 入れ替えの際、実行中の `hex.exe` は `hex.exe.upgrade-old` に退避してから差し替えます（古いファイルは次回以降の実行時に削除を試みます）。配置フォルダーへの書き込み権限が必要です。Hex 本体が起動中だと `speech.exe` / `speech.dll` のロックで入れ替え前に停止します。

## GPUなしのCPUバリアント（Windows x64 / ARM64）

Vulkanドライバーや `vulkan-1.dll` がないPCには、別パッケージ
`Hex-Haas-<version>-windows-x64-cpu.zip`（x64）と
`Hex-Haas-<version>-windows-arm64-cpu.zip`（ARM64）を使います。`hex-v*` タグの Release には
GPU版と一緒に両CPU版 zip も登録されます（SHA256SUMS は3版をまとめた1ファイル）。
単体確認用に Actions の「Haas CPU build」手動実行（artifactのみ）も残しています。
上のGPU版とは別の成果物です。CPU版の3ファイルをまとめて展開し、
`dist/haas-cpu/hex.exe`（zipでは `hex.exe`）を直接起動します。

必要なのは Windows 10/11 x64 と、2013年以降の CPU（Haswell / Excavator 以降、
AVX2 対応）です。v3.3.0の実検査はAVX2・BMI2・FMA・F16C（x86-64-v3）です。CPU推論はGPU版より低速です。
画面描画（Rune/wgpu、DX12 WARP等）は別で、AVDでの表示は実機確認が必要です。

```powershell
./scripts/setup-haas-cpu-build-tools.ps1
./scripts/build-haas-cpu.ps1
./scripts/build-haas-cpu.ps1 -Check
./scripts/build-haas-cpu.ps1 -Lint
./scripts/build-haas-cpu.ps1 -TestFilter haas
./scripts/package-haas-cpu.ps1
./dist/haas-cpu/hex.exe --version
./dist/haas-cpu/hex.exe model devices
```

ビルドの前提ツールはGPU版と同じですが、Vulkan SDK・shaderc・GPUドライバーは不要です。
CPU用ツール・speech.cppのソースとビルド結果は `tools/haas-cpu/`、Rust/native成果物は
`target-cpu/`、実行ファイルは `dist/haas-cpu/`、zipは `dist/package-cpu/` に分離します。
`tools/haas-cpu/` と `target-cpu/` もGitから除外しています。
CPUスクリプトはGPU用環境変数と `TRANSCRIBE_DIR` を継承・再利用しません。
同じPowerShellで後からGPU版をビルドする場合は、新しいシェルを使ってください。

Windows x64のCargo既定もCPUになります。Vulkanを有効にするのは既存の
`build-haas.ps1`（Check/Lint/Testも含む）です。未使用の上流汎用入口
（直接のcargo、`build-windows.sh`、上流Windowsチェック・Release workflow）は
CPU構成になるため、HaasのGPU版には使わないでください。LinuxのVulkan指定は不変です。

設定・履歴・モデルはGPU版と同じ `%APPDATA%\hex-haas` と `%LOCALAPPDATA%\hex-haas`、ユーザー設定は
`~/.config/hex-haas` です。切り替える前にHexを終了し、GPU版のexe/DLLを混ぜないでください。
CPU版もコード署名なしで、モデルは初回にダウンロードします。更新はGPU版と同じく `hex haas-upgrade` で行います。

## ARM64 CPUバリアント（Windows ARM64）

Snapdragon X 搭載Surfaceなど Windows 11 ARM64 PC向けのネイティブARM64ビルドです。
x64エミュレーションではなくaarch64バイナリーで、x64 CPU版のAVX2前提（x86-64-v3）
チェックはARM64では動きません。推論はCPUのみで、Vulkanは不要です。ARM64のGPU
（Vulkan）版は存在しません。speech.cppのVulkan対応に必要なshadercにはARM64
Windows版リリースがないためです。設定・履歴・モデルはx64版と同じ保存先です。

ggmlのARMバックエンドはMSVCを拒否するため、上流のWindows ARM64構成と同じく
clang-cl + Ninjaでネイティブコアをビルドします（`GGML_NATIVE=OFF`、OpenMPなし）。
CMake 4.4.5・Ninja 1.12.1・LLVM 20.1.8（ARM64 Windows版clang-cl）を固定バージョンで
取得しハッシュ検証します。LLVMがARM64バイナリーのため、ビルドはARM64 Windows
ホスト専用です（他アーキテクチャーのホストではスクリプトが止まります）。CIは
`windows-11-arm` ランナーで行います。

```powershell
./scripts/setup-haas-cpu-build-tools.ps1 -Arch arm64
./scripts/build-haas-cpu.ps1 -Arch arm64
./scripts/build-haas-cpu.ps1 -Arch arm64 -Check
./scripts/build-haas-cpu.ps1 -Arch arm64 -Lint
./scripts/build-haas-cpu.ps1 -Arch arm64 -TestFilter haas
./scripts/package-haas-cpu.ps1 -Arch arm64
./dist/haas-cpu-arm64/hex.exe --version
./dist/haas-cpu-arm64/hex.exe model devices
```

ARM64用のツール・ネイティブ成果物・実行ファイルは `tools/haas-cpu-arm64/`、
`target-cpu-arm64/`、`dist/haas-cpu-arm64/`（zipは `dist/package-cpu/`）に分離し、
Gitから除外しています。MSVC ARM64ビルドツール（link.exe・インポートライブラリー・
Windows SDK）はVisual Studio側から使います。詳細な検証定義は
[docs/haas-ja.md](docs/haas-ja.md) のARM64節を参照してください。

CIには全バイナリのdumpbin検査（loader不在の証明はこのimport走査が担う。ARM64は
さらにmachine型がARM64であることの検査も行う）、CLI起動・デバイス列挙、
Haasモデル取得と非無音の合成WAV推論を必須ゲートとして用意しています。
日本語SAPI音声には依存せず、日本語の精度は検証しません。モデル取得・推論失敗を
許容してartifactを出すことはありません。これらは実行前の検証定義であり、
2026-11-04にCIで両ワークフローとも成功し、WindowsビルドとGPU版リンク回帰は確認済みです。未検証なのはAVDの画面・実口述・貼り付けです。
