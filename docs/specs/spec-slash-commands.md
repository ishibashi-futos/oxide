# スラッシュコマンドモード

`/` で始まる入力をトリガーとして、Ox 内で即座に状況を変える軽量な操作をまとめて提供するモードです。普段の A-style ナビゲーションとは別に「コマンドを打ち込む時間」を意識的に作ることで、既存のキー操作に新しい軸が生まれます。

## ユーザーストーリー

### 1. スラッシュ入力バーを瞬時に呼び出せる

**ユーザーの価値:** `/` を押しただけで特別なコマンドラインが現れると、キーボードから手を離さずに状況を変えられる。

**受け入れ条件:**
- [x] **Given** 通常時は `search:` ラインだけ表示されている状態で、**When** ユーザーが `/` を押す、**Then** 背景色が異なる入力ラインが下部に現れ、`/command [args...]` にフォーカスされる。
- [x] **Given** スラッシュコマンドバーが表示されている、**When** `ESC` または `Ctrl+C` を押す（または `/` を打たずにキャンセル）、**Then** 入力ラインが消えて `search:` ラインだけに戻る。
- [x] **Given** スラッシュコマンドバーが表示されている、**When** 任意のキーを入力して `Enter` を押す、**Then** UI が `SlashCommand { name, args }` を送信し、Core から `SlashFeedback` を受け取る。

### 2. `/preview` や類似コマンドでプレビュー制御ができる

**ユーザーの価値:** ファイル閲覧中にプレビューのオン/オフをすばやく切り替えられると、ノイズになるペインを閉じて集中できる。

**受け入れ条件:**
- [x] **Given** プレビューが表示されている状態、**When** `/preview` を実行、**Then** `preview: off` を Bottom Bar に表示し、プレビュー領域が折りたたまれる。
- [x] **Given** プレビューが非表示、**When** `/preview show` を実行、**Then** プレビュー領域が復帰し `preview: on` を表示する。
- [x] **Given** プレビュー領域の表示状態にかかわらず、**When** `/preview hide` を実行、**Then** Core は `PreviewRequest` を一時停止し、表示が即座に閉じる。
- [x] **Given** `/preview` 系コマンドを実行すると、**When** コマンド成功、**Then** Bottom Bar に `preview: on/off` のステータスが出てバッジ色と同期する。

### 3. テキスト系ファイルのプレビューを即座に確認したい

**ユーザーの価値:** 外部アプリを開かずにテキストを確認できると、探しているファイルか判断しやすい。

**受け入れ条件:**
- [x] **Given** TXT/MD/Log/CSV などのテキスト系ファイルを選択し、**When** Core が `PreviewRequest` を受け取る、**Then** `PreviewLoading` → `PreviewReady` で先頭最大 40 行を折り返しつつ表示し、行末に `…` を付ける。
- [x] **Given** ファイルが 1 MiB を超えるか UTF-8 以外の場合、**When** Core が読み込みを試みる、**Then** `PreviewFailed { reason }` か `PreviewReady` の `reason` に `Some("非UTF-8のため簡易モード")` を含めて UI に伝える。
- [x] **Given** Markdown ファイルの読み込み、**When** Core が `#`, `-`, ```` ``` ```` などを検出すると、**Then** `LineKind` を添えて UI に渡し、見出しやコードブロックを提示する。
- [x] **Given** `PreviewReady` を UI が受け取る、**When** プレビュー表示が有効、**Then** 右ペイン/下部に `content preview` ブロックを更新し、非対象ファイルでは理由文言を表示する。

### 4. 履歴と補完でコマンド入力を高速化したい

**ユーザーの価値:** 以前使ったコマンドや候補をすばやく再利用できると、体験がストレスフリーになる。

**受け入れ条件:**
- [x] **Given** セッション中に複数のスラッシュコマンドが実行済みで、**When** 上下矢印 / `Ctrl+P` / `Ctrl+N` を押す、**Then** 過去の入力が順番にコマンドバーへ表示される。
- [ x **Given** コマンドバーに `/p` まで打った状態、**When** 候補表示が出ている、**Then** 候補リスト（`/preview`, `/paste` など `p` 始まり）を見て `Tab` で補完できる。
- [x] **Given** 候補が選択されて `Enter` した、**When** コマンド成功、**Then** `SlashFeedback { text, status }` と履歴が同期され、Bottom Bar に結果表示が出る。

### 5. プレビュー表示領域の幅を動的に調整したい

**ユーザーの価値:** プレビューを表示／非表示にしたとき、Active ペインと Preview ペインの割合が自動で切り替わると、画面スペースを無駄にせず視線移動が減る。

**受け入れ条件:**
- [x] **Given** `/preview show` 実行直後、**When** Core が `PreviewReady` を返す、**Then** プレビューが表示されると同時に Active/Preview の横幅比率を再計算し、Preview 側を 30〜40% 程度に設定する。
- [x] **Given** `/preview hide` を実行、**When** Core が `PreviewFailed` や非表示イベントを返す、**Then** Active ペインを最大化して Preview 幅を 0 にし、画面がプレビューなし表示にシームレスに移行する。
- [x] **Given** プレビュー表示中に `/preview` でトグルすると、**When** 状態が変わる、**Then** Core が現在のタイル比率を記憶し、再表示時には前回の幅比を復元しつつ `PreviewReady` を再取得する。


## 技術的な責務とメッセージ設計

- **UI**
  - `SlashInputActivated`, `SlashInputCancelled`, `SlashCommandSubmitted { name, args }` を生成。
  - 入力中は通常ペインのキーバインドを抑制し、`SlashCandidates { items }` で補完候補を表示する。
- `SlashFeedback { text, status }` を Bottom Bar のステータス行に流し、履歴とバッジの色を同期。ステータス行はメタデータ行の上に位置するため、`preview: on/off` のようなメッセージを表示しても metadata が常に見える状態にする。
- **Core**
  - `SlashCommand` を受け取って `PreviewToggle`, `ShellScriptStart` などの内部イベントに分解。
  - `PreviewRequest` を処理し、`PreviewLoading / PreviewReady / PreviewFailed` を返す。
  - コマンドごとの Permission / Context チェックを行い、失敗は `SlashFeedback { text: "権限なし", status: Error }` で通知。

### プレビュー向けメッセージ

UI -> Core

- `PreviewRequest { id, path: PathBuf, max_bytes: usize }`

Core -> UI

- `PreviewLoading { id }`
- `PreviewReady { id, lines: Vec<String>, truncated: bool, reason: Option<String>, kind_flags: Vec<LineKind> }`
- `PreviewFailed { id, reason: PreviewError }`

`PreviewError` は `TooLarge`, `BinaryFile`, `IoError(String)`, `PermissionDenied` 等。

### 読み込み方針

1. `max_bytes` 分を読み込み、UTF-8 変換に成功したら行単位に切り、40 行で `truncated = true`。
2. Markdown の見出し/リスト/コードブロックを検出して `LineKind` を付与。
3. 混在する文字列には `reason: Some("部分的に非UTF-8")` を添える。
4. `/preview hide` 状態では `PreviewRequest` を抑制し、再表示時に最新ファイルで再発行する。

## TODO

- [x] `preview: on/off` の表示が Bottom Bar のメタデータを隠さないようにする。
  - たとえばメタデータより1段上に専用行を追加して表示する案を検討する。
- [x] `preview failed: preview: Is a directory` の文言を改善する。
  - 例: `No Content: Is a directory` のように理由が分かる短い表現にする。

## TDD 最小ステップ

1. `SlashCommand` パーサーのユニットテスト。`/preview`, `/preview hide`, `/preview show` で `CommandName` と `args` が正しく分かれることを確認。
2. `preview::load_chunk` の単体テストでテキスト／バイナリ／サイズオーバーの分岐と `LineKind`/`reason` を含める。
3. Core が `PreviewRequest` を処理し、`PreviewReady / PreviewFailed` を生成する統合テスト。
4. 過去履歴・補完候補・`SlashFeedback` を含めた UI のエンドツーエンドテストで表示状態と Bottom Bar の同時更新を確認。
