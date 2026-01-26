# ユーザー通知・フィードバック共通仕様

この文書は、Ox 内の「成功/失敗/警告/情報」通知を共通化するための仕様です。

スラッシュコマンド、`/shell`、セッション保存失敗など、複数機能で同じUI経路を使います。


## 1. 目的

- ユーザーが「今なにが起きたか」を短く理解できるようにする。
- 成功/失敗/警告/情報の表現を一貫させる。
- UI をブロックしない通知経路を標準化する。

## 2. 用語

- **Notice**: ユーザーに見せる短い通知メッセージ。
- **Level**: `Info` / `Success` / `Warn` / `Error` の4種類。
- **Source**: どの機能からの通知かを示すラベル。

## 3. 表示ルール

- 通知は Bottom Bar のステータス行に表示する。
- 1行で完結する短文にする。
- 重要度に応じて色を変える。
- 連続通知は最新を表示する。
- 先頭にレベルごとのアイコン（絵文字）を付ける。
- `/shell` を含む既存のボトム表示は、この通知表示に統一する。


## 4. 通知の優先度

- `Error` が最優先。
- `Warn` は `Error` の次に優先。
- `Success` と `Info` は最後に上書きされる。


## 5. 消えるタイミング

- `Success` / `Info` は TTL（例: 4秒）で消える。
- `Warn` / `Error` は一定時間残すが、次の `Warn` / `Error` で上書きする。

## 6. Core → UI の通知イベント

将来的に共通イベントとして扱う。

```text
UserNotice {
  level: Info|Success|Warn|Error,
  text: String,
  source: String,      // "shell", "session", "tab" など
  ttl_ms: Option<u64>, // None は明示的に残す
}
```

UI は `UserNotice` を受け取り、Bottom Bar に表示する。

### 6.1 レベルとアイコンの対応

- `Success`: ✅
- `Info`: ℹ️
- `Warn`: ⚠️
- `Error`: ❌

表示例: `✅ shell: exit=0`

## 7. 既存機能との対応

### 7.1 スラッシュコマンド

- 現在の `SlashFeedback { text, status }` は `UserNotice` へ変換できる。
- `status: Success/Error/Warn` を `level` に対応させる。


### 7.2 `/shell`

- 実行開始: `Info`（例: `shell: ls -al...; started`）。
- 実行成功: `Success`（例: `shell: cargo bui...; exit=0`）。
- 実行失敗: `Error`（例: `shell: docker ru...; exit=1`）。


### 7.3 セッション保存

- 保存失敗: `Warn` または `Error`。
- 例: `session: save failed (set OX_CONFIG_HOME)`。
- 成功通知は原則出さない。

## 8. 受け入れ条件

- `UserNotice` が UI に到達し、Bottom Bar に表示される。
- 重要度に応じて色が変わる。
- TTL で自動的に消える。
- `Error` が表示中でも UI 操作は継続できる。


## 9. TODO

- [x] `UserNotice` の共通イベント実装。
- [x] `SlashFeedback` から `UserNotice` への統合。
- [ ] セッション保存の失敗通知実装。

### 公開仕様

- 呼び出し側は `UserNotice` を作って渡す。
  - `level`、`text`、`source` が必須。
  - `ttl_ms` は必要に応じて指定する。
- `UserNotice::new` は全レベルの既定 TTL を `4000ms` にする。
- `Warn`/`Error` でも長く残したい場合は `UserNotice::with_ttl_ms` で TTL を長くするか、`None` を指定して残す。
- 既定と違う TTL にしたい場合は `UserNotice::with_ttl_ms` を使う。
- `UserNoticeQueue::push` は優先度に従って置き換える。
  - `Error` > `Warn` > `Success`/`Info`。
  - 同じ優先度なら上書きする。
- `UserNoticeQueue::current` は TTL を見て自動で消す。
