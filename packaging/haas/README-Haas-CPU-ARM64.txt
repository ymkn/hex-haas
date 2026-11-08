Hex Haas — Windows ARM64 CPU（非公式フォークビルド）
====================================================

公式Hex（https://github.com/anomalyco/hex, MIT License, (c) 2026 Christopher Langton）
に、日本語音声認識モデル Haas-ja-v1.0 対応を加えたフォークのARM64ネイティブCPU版です。
公式のリリースではありません。

起動
  zipを展開して hex.exe を直接実行してください。初回はウィンドウが開きます。
  同梱の speech.exe / speech.dll はARM64用にビルドした speech.cpp v0.10.0
  （MIT License, (c) 2026 Sakasegawa, https://github.com/nyosegawa/speech.cpp）
  です。3ファイルを同じフォルダーに置き、他の版（x64 GPU/CPU）のファイルを
  混ぜないでください。

使い始め
  1. Windowsの設定でマイクへのアクセスを許可する
  2. Hexの General > Speech model から Haas-ja-v1.0 (speech.cpp) を選び、
     モデルをダウンロードする（huggingface.co に接続します）
  3. 設定したショートカットを押しながら話し、離すと認識結果が貼り付きます

必要なもの
  - Windows 11 (ARM64) 搭載PC（Snapdragon X搭載Surfaceなど）
    x64エミュレーションではなくARM64ネイティブバイナリーです。
  - 推論にGPU・Vulkan・AVX2は不要です。x64 CPU版のAVX2前提（x86-64-v3）
    チェックはARM64では行われません。
  - CPU推論のためGPU版より低速です。

データの保存先
  設定・履歴 : %APPDATA%\hex-haas
  モデル・ログ : %LOCALAPPDATA%\hex-haas
  hex.config.ts      : %USERPROFILE%\.config\hex-haas
  他の版（x64 GPU/CPU）と同じ保存先を共有します。切り替える前に起動中のHexを
  終了してください。公式Hexのデータとは混ざりません。

注意
  - 自動更新・コード署名はありません。更新はこのフォルダーで hex haas-upgrade を実行してください。SHA256SUMSでzipのハッシュを確認してください。
  - 初回起動時にSmartScreenの警告が出ることがあります。
  - Haasモデル（Apache-2.0）は同梱していません。起動後にダウンロードされます。
  - ARM64 CPU版はhex-v*タグのReleaseにx64 GPU/CPU版と一緒に登録されます。
    Actionsの手動実行artifactでも取得できます。
  - ライセンス本文は同梱の LICENSE を参照してください。
