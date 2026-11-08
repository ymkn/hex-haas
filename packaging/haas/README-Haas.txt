Hex Haas — Windows x64（非公式フォークビルド）
================================================

公式Hex（https://github.com/anomalyco/hex, MIT License, (c) 2026 Christopher Langton）
に、日本語音声認識モデル Haas-ja-v1.0 対応を加えたフォークのビルドです。
公式のリリースではありません。

起動
  hex.exe を直接実行してください。初回はウィンドウが開きます。
  同梱の speech.exe / speech.dll は Haas の認識エンジン speech.cpp v0.10.0
  （MIT License, (c) 2026 Sakasegawa, https://github.com/nyosegawa/speech.cpp）
  で、hex.exe と同じフォルダーに置いてください。

使い始め
  1. Windowsの設定でマイクへのアクセスを許可する
  2. Hexの General > Speech model から Haas-ja-v1.0 (speech.cpp) を選び、
     モデルをダウンロードする（huggingface.co に接続します）
  3. 設定したショートカットを押しながら話し、離すと認識結果が貼り付きます

必要なもの
  - Windows 10 / 11 (x64)
  - Vulkan対応GPUドライバー（NVIDIA / AMD / Intel製。vulkan-1.dll が
    システムにあれば十分です）

データの保存先
  設定・履歴 : %APPDATA%\hex-haas
  モデル・ログ : %LOCALAPPDATA%\hex-haas
  hex.config.ts      : %USERPROFILE%\.config\hex-haas
  公式Hexのデータとは混ざりません。

注意
  - このビルドは自動更新しません。更新は Releases から取り込み直してください。
  - コード署名がないため、初回起動時にSmartScreenの警告が出ることがあります。
  - Haasモデル（Apache-2.0）は同梱していません。起動後にダウンロードされます。
  - ライセンス本文は同梱の LICENSE を参照してください。
