# /shell コマンド仕様

この文書は `/shell {command}` に関する目的・必須事項・安全要件を、「段階的に開示する」形で整理したものです。まずコマンドの存在意義と必須動作を概説し、その後に細かいカテゴリ（入力サニタイズ、権限制御、実行フロー、UI フィードバック）を順次掘り下げていきます。実装にあたって矛盾や曖昧な指示があれば明示し、不要なものは `## 削除予定` にまとめています。

## 1. 最小実装の意図

- **目的:** 現在のタブの `working_dir` 下で、ユーザーが `/shell {command}` という一行で任意の外部コマンドを実行できるようにする。
- **必須:**
  1. `/shell` 発行で `SlashCommand` が Core に届き、`ShellCommandRequest` に変換できること。
  2. `std::process::Command` を使ってシェルを起動できること（`env_clear()` など環境を絞るのはこの段階でも行う）。
  3. 実行結果は `ShellExecutionResult { status_code, stdout, stderr }` を `SlashFeedback` で Bottom Bar に要約して返す。
- **TDD着手点:** `/shell ls` のような入力をパースし、`ShellCommandRequest` にマッピングするテストを最初に書く。

## 2. 安全性と入力制御

### 2.1 サニタイズと構文制限
- 生文字列で `&&`, `;`, `|`, `$(`, `` ` `` などのシェル結合パターンを含む場合は即時拒否し、`SlashFeedback { status: Error, text: "連結演算子は禁止" }` を返す。これは `/shell` の入力欄で `ShellCommandParser::sanitize_args` が担当。
- 明示的なクォートを本当に必要とするケースがあれば、クォートを維持したままトークン分割し、`ProgramArgs` を再構成するテストを追加（正しいパース/拒否の両方）。
- ファイルパスなどの引数があれば `current_dir.join(arg)` で `PathBuf::canonicalize` し、`working_dir` からはみ出すものは拒否して脱出を防ぐ。

### 2.2 権限チェッカー
- `/shell` を動かせるのは明示的に `OX_ALLOW_SHELL` が真か `./ox-config.json` などで許可されたユーザーだけとする `ShellPermission` モジュールを通す。
- 権限がない場合は `SlashFeedback { status: Warn, text: "Shell commands disabled" }` で UI に通知し、Core は実行を拒否。

## 3. 実行フローと環境制御
- 実行前の準備は `ShellExecutionGuard` に委ね、次のチェックを通過したものだけを `Command::new(shell_path)` へ流す：
  1. 許可されたシェルパスであること（今は `sh` / `pwsh` のデフォルトのみ）；
  2. `AllowedShell` の列挙体を将来的に拡張できるようにしつつ、当面は「OS に依存しない default shell」のみサポートする（詳細は docs/plan.md の TODO）。
  3. `env_clear()` で親環境を消去し、`OX_SAFE_ENV` のみを再注入。これにより密結合な環境変数注入を避ける。
- 実行結果は `ShellExecutionResult` として `status_code`・`stdout`・`stderr`・`duration_ms` を含め、UI は `shell: exit=0 stdout=...` など人間が読める要約を Bottom Bar に表示する（`stdout` は省略または `PreviewPane` に送るかは将来議論）。

## 4. UI メッセージと履歴
- UI から Core へのメッセージ：`SlashCommandSubmitted { name: "shell", args }`。
- Core の応答：`SlashFeedback { text, status }` や `ShellExecutionResult` を送る。成功時は `status: Success`、失敗時は `status: Error` 。
- 履歴・候補は `SlashCandidates` まわりと同期し、エラー/警告も Bottom Bar に残すことで `rm -rf` 型のコマンド結果をユーザーが即座に確認できる。
- `/shell` 起動時は `ShellCommandRequest::working_dir` も明示し、どのタブでコマンドが走ったかを UI で明示して混乱を避ける。
- `/shell` 入力には Path Intellisense を効かせ、スペース区切りの最後のトークンだけを対象に補完する。候補は `./build.sh ./di` のように続けた場合は 2 番目の `./di` に対して表示し、Tab で候補確定すると続く値の末尾にスペースを自動挿入して次の引数入力に移れるようにする。

## 5. Shell 出力のプレビューとバッファ制御
- 実行結果の詳細を確認するため、`ShellExecutionResult` を UI が `ShellOutputView` へ転送できるようにする。これは `less` 風に上下移動できるビューで、複数行出力をスクロールして確認できる。
- `ShellOutputView` は `stdout`/`stderr` を行単位で `VecDeque<String>` に保持し、`scroll_offset` を持って上下キーで表示範囲を変化させる。ビューは必要に応じて `PreviewPane` を再利用し、「shell output」ブロックを追加して表示することが望ましく、少なくとも Bottom Bar からキーバインド（例: `Ctrl+O`）で開けるようにする。
- 閉じる操作は `ESC` を共通ルールとし、`ShellOutputView` がアクティブなときは `Ctrl+O` の再押下または `ESC` でビューを閉じる。
- バッファの上限は一切のビルド・テスト・パッケージング出力を収め得るサイズを想定しつつ、メモリに過度な負担をかけないよう以下を基本とする：
  1. `ShellOutputView::max_bytes` を 2MiB 程度（= 2_097_152 バイト）に設定し、`stdout`/`stderr` をストリーミングしながら超過分は最初の行から破棄する（リングバッファ）。
  2. `max_lines` は 4,000 行前後にして、行長が短ければ数千行、多めのビルドログが収まるようにする。
  3. `duration_ms` や `timestamp` を伴って `ShellExecutionResult` を送ることで、UI が「いつ」「どれくらいかかったか」も表示できる。
  4. バッファサイズは設定可能（`ox-config.json` で `shell_output_limit_bytes` など）とし、ビルドログがさらに長い場合でもユーザーが調整できるようにしておく。
- バッファの保持期間は特に設けず、セッション（`ox` の再起動まで）が続く限り `ShellOutputView` は最後に実行した `/shell` のログを保持する。
- 新しい `/shell` コマンドを起動したタイミングでバッファはリセットし、常に直近の実行結果だけをスクロール可能にしてメモリを節約する。
- これらを実装する際はメモリ使用量の実測（`cargo bench` など）で 2MiB/4,000行という仮定が安定するか確認し、必要に応じて上限を下げるかストレージに退避する方式も検討する。

## 6. TDD最小ステップ
1. [x] サニタイズテスト: `&&`, `;`, `|` を含む `/shell` 入力を拒否する。
2. [x] パーステスト: `/shell "echo foo"` や `/shell ls -a` が `Vec<String>` へ正しく分解される。
3. [x] 実行ガードテスト: temp ディレクトリで `echo hi` を起動し、`OX_SAFE_ENV` のみで `stdout` が得られるか確認。
4. [x] 権限テスト: `OX_ALLOW_SHELL=false` で `SlashFeedback` が警告となり、実行しないことを確認。
5. [x] UI 統合テスト: `SlashFeedback` の表示が Bottom Bar と履歴に渡ることを確認し、`shell` 実行後の Bottom Bar 文字列が期待通りかを snapshot。
6. [ ] Path Intellisense実装&テスト: `/shell ./build.sh ./di` で最後のトークンだけが補完対象となり、Tab 確定でスペースが挿入されることを確認。新しいトークン完成後も既存ログが保持されることを検証。

## 7. 文書間リンク

- `docs/plan.md` の `/shell {command}` 項目と連動し、AllowedShell 機構（shell選択）を後回しにするTODOを維持。
- `/slash-commands.md` には `/shell` の補完・履歴・Bottom Bar 表示の基礎を追記する際、このセキュリティ・実行フローを引用する。

## 8. 削除予定

- 旧仕様で「Windows は pwsh 系、Unix は sh を優先し、ShellProfile を照合」という記述があったが、現在の最小実装では OS ごとの特化を盛り込みきれないため削除。将来的に `AllowedShell` で扱う際に再考する。
- 「悪意ある入力を完全に止める」ために `/shell \\\"$(rm -rf /)\\\"` のような例を列挙した記述は冗長。正規化と拒否ロジックで包括的に対処できるので、具体的なコマンド例は該当セクションへ統合できる。
- 「PreviewPane への出力転送を今すぐ決める」という指示は曖昧で、UI 側で議論が必要なため `## 削除予定` に残し、将来の検討事項として分離する。
