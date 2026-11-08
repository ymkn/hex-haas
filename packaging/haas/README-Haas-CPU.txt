Hex Haas — Windows x64 CPU（非公式フォークビルド）
================================================

公式Hex（https://github.com/anomalyco/hex, MIT License, (c) 2026 Christopher Langton）
に、日本語音声認識モデル Haas-ja-v1.0 対応を加えたフォークのCPU版です。
公式のリリースではありません。

起動
  zipを展開して hex.exe を直接実行してください。初回はウィンドウが開きます。
  同梱の speech.exe / speech.dll はCPU用にビルドした speech.cpp v0.10.0
  （MIT License, (c) 2026 Sakasegawa, https://github.com/nyosegawa/speech.cpp）
  です。3ファイルを同じフォルダーに置き、GPU版のファイルを混ぜないでください。

使い始め
  1. Windowsの設定でマイクへのアクセスを許可する
  2. Hexの General > Speech model から Haas-ja-v1.0 (speech.cpp) を選び、
     モデルをダウンロードする（huggingface.co に接続します）
  3. 設定したショートカットを押しながら話し、離すと認識結果が貼り付きます

必要なもの
  - Windows 10 / 11 (x64)
  - 2013年以降の CPU（Haswell / Excavator 以降、AVX2 対応）
    v3.3.0の実検査はAVX2・BMI2・FMA・F16C（x86-64-v3）です。
    AVDでは仮想CPUにもこれらの命令セットが必要です。
  - 推論にGPU・Vulkan SDK・vulkan-1.dllは不要です。
    CPU推論のためGPU版より低速です。
  - 画面描画（Rune / wgpu、DX12 WARP等）は別です。AVDで画面が表示できるか、
    マイク・ショートカット・貼り付けが動くかは実機で確認してください。

データの保存先
  設定・履歴 : %APPDATA%\hex-haas
  モデル・ログ : %LOCALAPPDATA%\hex-haas
  hex.config.ts      : %USERPROFILE%\.config\hex-haas
  GPU版と同じ保存先を共有します。切り替える前に起動中のHexを終了してください。
  公式Hexのデータとは混ざりません。

注意
  - 自動更新・コード署名はありません。更新はこのフォルダーで hex haas-upgrade を実行してください。SHA256SUMSでzipのハッシュを確認してください。
  - 初回起動時にSmartScreenの警告が出ることがあります。
  - Haasモデル（Apache-2.0）は同梱していません。起動後にダウンロードされます。
  - CPU版はhaas-v*タグのReleaseにGPU版と一緒に登録されます。Actionsの手動実行artifactでも取得できます。
  - ライセンス本文は同梱の LICENSE を参照してください。
