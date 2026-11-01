# Hex Haas fork

日本語音声認識モデル Haas-ja-v1.0 をWindows版Hexから使うための個人用Forkです。公式Hexのリリースではありません。

- Fork: https://github.com/ymkn/hex
- 上流: https://github.com/anomalyco/hex
- 改修ブランチ: `haas`
- ベース: 公式 `v3.0.0` (`7b05b5fd1fa0e3b9c333209cd1644d9603693d5c`)
- `dev` は上流用として残し、個人の変更は `haas` にだけコミットします。

Haasは speech.cpp v0.10.0 の常駐worker経由で動きます。Whisperなど既存モデルは元の認識処理を使います。15秒以内の音声は分割せず、Haas自身の句読点出力を保持します。詳細・制約・検証結果は [docs/haas-ja.md](docs/haas-ja.md) を参照してください。

## ローカル作業場所とリモート

現在の作業場所は `~/source/repos/hex-haas` です。Gitのリモートは次の役割に分けます。

```text
origin   -> ymkn/hex          自分の変更の取得・push
upstream -> anomalyco/hex     公式更新の取得のみ
```

`remote.pushDefault=origin`、`pull.ff=only` を設定し、`upstream` へのpush URLは意図的に無効化しています。これらのGit設定はclone先へ自動継承されません。別PCでは以下で構成します。

```powershell
git clone --branch haas https://github.com/ymkn/hex.git hex-haas
Set-Location hex-haas
git remote add upstream https://github.com/anomalyco/hex.git
git remote set-url --push upstream https://example.invalid/upstream-push-disabled
git config remote.pushDefault origin
git config pull.ff only
```

## 上流の変更を取り込む

自動更新は無効です。公式更新の取得、ソースへのマージ、ビルド、実行ファイルの入れ替えは別々に行います。公式をfetchしただけでは、使用中のHexは変わりません。

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

特に `transcription_models`、`local_transcriber`、`experiments`、`app_paths`、`Cargo.toml` の変更を確認してください。上流のファイル移動やAPI変更には手動適応が必要です。公式のRune/native依存ピンは、単に「最新」という理由で個別に変えず、選んだ上流との互換性を検証します。

```powershell
./scripts/build-haas.ps1 -Check
./scripts/build-haas.ps1 -Lint
./scripts/build-haas.ps1 -TestFilter haas
./scripts/build-haas.ps1 -BuildOnly

# この後、起動中の試験版をトレイから終了する。
Copy-Item ./target/release/hex.exe ./dist/haas/hex.exe -Force
./scripts/test-haas-package.ps1
uv run --no-project python ./scripts/check_haas_punctuation.py
git push origin haas
```

公開音声や合成音声によるテストと、実マイク・ショートカット・貼り付け確認は区別してください。既存の全workspaceテストにはWindows非対応の `/bin/sh` 前提テストが1件あり、詳細は検証記録に記載しています。

## 再構築とデータ

Visual Studio x64 C++ツール・Windows SDK・Rust・PowerShell 7を用意し、[ビルドガイド](docs/haas-ja.md#build-and-launch) の固定バージョンに従ってspeech.cppを取得、ハッシュ検証してから展開します。

```powershell
./scripts/setup-haas-build-tools.ps1
./scripts/build-haas.ps1
./Start-Hex-Haas.cmd
```

Haasモデルは初回セットアップかモデル選択から取得します。通常のHexが既にあるPCではランチャーが設定を別フォルダーへコピーしますが、モデル本体はGitからは復元されません。試験版は `haas-data/` と `haas-config/` を使い、公式Hexのデータは変更しません。

GitHubへ保存するのはソース・スクリプト・文書だけです。次は `.gitignore` で除外しています。

- `haas-data/`、`haas-config/`: 個人設定、履歴、ログ、モデル
- `tools/haas/`: ダウンロードしたツール、検証音声、旧バイナリのバックアップ
- `target/`、`dist/`: ビルド結果と実行ファイル

これらの除外フォルダーは必要に応じて別途バックアップしてください。Forkは上流のライセンスを保持します。上流への提案は、外部PRを受け付けない公式の方針に従ってIssueを使います。
