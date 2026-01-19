# 3.1 ドラッグ＆ドロップによるコピー

目的は、ターミナルへドロップされたパス文字列を安全に解釈し、コピーを開始することです。

UIは入力を受け、Coreはコピー処理だけを担当します。

## 対象OSと想定入力

対象は macOS と Windows です。
macOS の Terminal では、ドロップすると絶対パスがコマンドラインへ挿入されます。
スペースや括弧はバックスラッシュでエスケープされることが多いです。
Windows の PowerShell / CMD では、スペースを含むパスはダブルクォートで囲まれることがあります。

### 想定される入力例

- `/Users/a/My\ File.txt`
- `/Users/a/My\ File\ (1).txt`
- `"C:\Users\a\My File.txt"`
- `C:\Users\a\MyFile.txt`

## 入力の検出

`crossterm` の `Event::Paste` を優先して使います。
Paste が無い場合は、短時間に大量の文字が入ったときだけ「ドロップ候補」とみなします。

## パース方針

1. 先頭末尾の空白を削る。
2. 先頭と末尾が同じクォートなら剥がす。
3. macOS の場合、バックスラッシュによるエスケープを外す。
4. OSに応じた Path として正規化する。
5. 存在確認に失敗したら UI にエラーを返す。

macOS のエスケープ解除は、`\ `, `\(`, `\)`, `\[` などを対象にします。

Windows ではクォートの除去が最優先です。

## Core / UI 分離

UI は「パス文字列の検出と整形」までを担当します。
Core は「コピーの実行」と「進捗の通知」だけを担当します。

## メッセージ設計案

UI -> Core:

- `CopyRequest { sources: Vec<PathBuf>, dest_dir: PathBuf }`

Core -> UI:

- `CopyStarted { id }`
- `CopyProgress { id, done_bytes, total_bytes }`
- `CopyFinished { id }`
- `CopyFailed { id, message }`

## 進捗表示

Bottom Bar に「copy: 20%」のような短い表示を置きます。
コピー対象が複数なら、合計バイトで算出します。

## エラー方針

存在しないパスは即座に拒否します。
権限エラーは UI に短い文言で返します。

## TDD の最小ステップ

まずはパース関数の単体テストだけを書くのが最小です。
次に、存在確認の失敗が UI メッセージへ伝わるテストを追加します。
