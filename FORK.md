# Hex Haas fork

日本語音声認識モデル Haas-ja-v1.0 をWindows版Hexから使うための個人用Forkです。公式Hexに代えて普段使いする前提です。公式Hexのリリースではありません。

- Fork: https://github.com/ymkn/hex-haas
- 上流: https://github.com/anomalyco/hex
- 改修ブランチ: `haas`
- ベース: 公式 `v3.1.1`（`v3.0.0` `7b05b5fd1fa0e3b9c333209cd1644d9603693d5c` に `v3.1.1` をマージ）
- `dev` は上流用として残し、個人の変更は `haas` にだけコミットします。

Haasは speech.cpp v0.10.0 の常駐worker経由で動きます。Whisperなど既存モデルは元の認識処理を使います。15秒以内の音声は分割せず、Haas自身の句読点出力を保持します。詳細・制約・検証結果は [docs/haas-ja.md](docs/haas-ja.md) を参照してください。

Haas対応以外の使い方は通常のHexと同じで、ログイン時起動も通常どおり使えます。保存先のディレクトリ名だけを `hex` から `hex-haas` に変えています。

- 設定・履歴・ログ・モデル: `%APPDATA%\hex-haas`
- `hex.config.ts`: `~/.config/hex-haas`

`dist/haas/hex.exe` を直接起動できます。専用の環境変数や起動先切り替えは不要です。ローカルビルドのため、公式のローカルビルドと同じく自動更新は行いません。

## ローカル作業場所とリモート

現在の作業場所は `~/Desktop/hex-haas` です。Gitのリモートは次の役割に分けます。

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

Haasモデルはモデル選択から取得します。モデルの推奨や初回セットアップの動作は通常のHexと同じです。ランチャーは起動するだけで、設定のコピーやHaasへの強制変更は行いません。設定とモデルはGitからは復元されません。

以前の `haas-data/` と `haas-config/` を引き継ぐ場合は、Hexを終了し、それぞれ `%APPDATA%\hex-haas` と `~/.config/hex-haas` に移します。移動先に既存データがある場合は上書きしないでください。通常版の `%APPDATA%\hex` と `~/.config/hex` は読み書きしません。

GitHubへ保存するのはソース・スクリプト・文書だけです。次は `.gitignore` で除外しています。

- `haas-data/`、`haas-config/`: 旧保存先（移行前の個人データを誤ってコミットしないため引き続き除外）
- `tools/haas/`: ダウンロードしたツール、検証音声、旧バイナリのバックアップ
- `target/`、`dist/`: ビルド結果と実行ファイル

これらの除外フォルダーは必要に応じて別途バックアップしてください。Forkは上流のライセンスを保持します。上流への提案は、外部PRを受け付けない公式の方針に従ってIssueを使います。
