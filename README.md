# ox (Oxide)

`ox` は、Rustで構築された、高速かつ軽量で、直感的な操作性を追求したクロスプラットフォーム対応のCLIファイルマネージャーを目指しています

## 🌟 特徴

* 高速ナビゲーション: 矢印キー / `[` `]` による直感的な階層移動（階層移動重視の設計）
* 非同期アーキテクチャ: Core層とUI層を分離し、大規模ディレクトリでもUIがフリーズしません
* OSのクリップボードと連携したコピー＆ペースト（Ctrl+C/V）
* ターミナルへのドラッグ＆ドロップによるファイルコピー
* インクリメンタル検索: ファイル名を入力するだけで即座に目的のアイテムへジャンプ
* 検索中はCurrentパネル下部に検索文字列を表示
* セッション復元: 終了時のタブとパスを記憶し、再起動後に即座に作業を再開
* `/` から始まるスラッシュコマンド機能を利用可能

## 🛠 技術スタック

* Language: Rust
* TUI Framework: [ratatui](https://github.com/ratatui-org/ratatui)
* Async Runtime: [tokio](https://github.com/tokio-rs/tokio)
* FS Watcher: [notify](https://github.com/notify-rs/notify)
* Event Handling: [crossterm](https://github.com/crossterm-rs/crossterm)

## ⌨️ 主なショートカット

| キー | アクション |
| --- | --- |
| `←` / `→` | 階層移動（戻る / 進む） |
| `[` / `]` | 階層移動（戻る / 進む） |
| `Enter` | フォルダを開く / ファイルをデフォルトアプリで実行 |
| `Ctrl + H` | 隠しファイルの表示/非表示切り替え |
| `Ctrl + C` / `V` | コピー / 貼り付け |
| `Ctrl + T` | 新しいタブの追加 |
| `Ctrl + Q` | アプリケーションの終了 |
| `ESC` | インクリメンタル検索のクリア |
| `Backspace` | インクリメンタル検索の1文字戻し |

## 🚀 開発の進め方

### 依存関係のインストール

```bash
cargo build

```

### テストの実行

```bash
cargo test

```

## 📄 ライセンス

MIT License
