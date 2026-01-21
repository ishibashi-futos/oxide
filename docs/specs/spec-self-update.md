# セルフアップデート (self-update) 機能設計

## 概要
- `ox` を GitHub に置いた最新版へ自動で差し替える `ox self-update` コマンドを提供する。
- 最小限の操作でローカルのバージョンを検出し、タグ・Release・アーティファクト・チェックサムを順に確認しながら安全に置き換える。
- 変更は小さな責務に分けてテスト駆動で取り組み、UI とはメッセージ（`SelfUpdateMessage` / `SelfUpdateStatus`）で分離する。

## 1. 抑えるべき基本要件
1. 現在実行中のバージョンを明示的に取得（埋め込み定数か `Cargo.toml::version`）。
2. GitHub API からタグ一覧を取得し、`semver` で最新を判定。タグ名が `v` は含むものの、解析できないものは無視する。
3. ローカルバージョンが最新より古ければ、差分確認 → `yes/no` プロンプト（`--yes` 省略） → アップデートという基本フロー。
4. 対象プラットフォーム用 Release アセットを選び、`sha256sums.txt` によるチェックサム検証を行い、失敗したら更新を中止して元バイナリを残す。
5. 更新前に現在の実行ファイルを `ox-v<version>` 形式でコピー/退避し、置き換えに失敗したときにこのバックアップから復元できる。
6. 成功したら新旧バージョンを通知（`ox --version` で確認できること）、失敗したらエラー詳細と cleanup を報告。

## 2. コアフロー（

### 2.1 タグ・バージョンの検出
- `https://api.github.com/repos/ishibashi-futos/oxide/tags` を呼び出し、`name` を `semver::Version` で解析。
- `Version::cmp` で最大値を拾い、現行バージョンと比較する。プレリリース扱いや `prerelease` 属性はセマンティックに判断する。
- `needs_update(cur, latest)` を小さな関数で定義し、プレリリース→通常リリースの順序やパッチ差分までテストする。

### 2.2 アーティファクト選定・ダウンロード
- 最新タグに対応する Release （`assets`）から `ox-{target}-{version}` の命名ルールにマッチする行を選ぶ。対象は `OS`/`ARCH`/`target triple` で決定し、見つからなければエラー。
- `reqwest` 等の HTTP クライアントでストリームを一時ファイル（`tempdir`）に書き込む。進捗表示は optional で `SelfUpdateStatus::Downloading` などを TUI に送る。
- `sha256sums.txt` を同じ Release から取得し、対象アーティファクトの行を抽出。`sha2` でローカルファイルのハッシュを計算して一致を確認。
- `sha256sums` が存在しない Release では警告し、`--force-checksum` のようなオプションか `--insecure-checksum` （危険）を使うまで継続しない。

### 2.3 原子的な置き換え
- `current_exe()` を得て、そのあるディレクトリに `ox-v<current>` を作成し、`std::fs::copy` か `rename` で退避。
- ダウンロード済みバイナリを `ox.new` などに移し、`std::fs::rename` で `ox` と入れ替える。Windows では `.exe` ロックに注意し、必要なら `replace_file` 風処理。
- 置き換え後は権限を維持し（`chmod +x`）、一時ファイルを削除。失敗したら元バイナリとバックアップを継続させる。
- `SelfUpdateStatus::Updated(old, new)` を TUI に送る。

## 3. リカバリ／ロールバック
### 3.1 ロールバックコマンド
- `ox self-update rollback` を CLI に用意し、実行可能ファイルと同じディレクトリ内の `ox-v*` を列挙してユーザーに選ばせる。
- 選択したバージョンの `sha256sums.txt` を GitHub Release から再取得し、ローカルのバックアップと照合。ネットワーク障害で取得できない場合は `--force` で強制実行し、警告を表示。
- 成功したら現在バイナリを `ox-v<current>.new` に退避し、選んだバックアップを `ox` にリネーム。置き換え後の再起動案内を出す。

### 3.2 ロールバックの検証
- チェックサム一致・不一致例、ネットワークエラー、`--force` の振る舞いをユニットテスト化。
- バージョン付きバックアップ名を生成・識別するロジックを `SelfUpdater` が持ち、複数回の更新でも衝突しないことを確保。

## 4. 証明書／ネットワーク制限対応
- Cloudflare Zero Trust や Palo Alto GlobalProtect による TLS 中間証明書の影響を考慮して、`reqwest::ClientBuilder` に CA ストア追加や検証無視のオプションを組み込む。
- CLI オプション
  - `--cacert-path <path>` / `OX_SELF_UPDATE_CACERT`: `.cer`/`.pem` 形式の証明書を持ち込み、`ClientBuilder::add_root_certificate` で信頼する。
  - `--insecure`: `danger_accept_invalid_certs(true)` を有効化。出力で明示的に警告し、自動化向けには `--insecure` の明示を必須とする。
  - `--offline --binary-path <path>`: すでにダウンロード済みのバイナリを指定し、チェックサムはローカルの `sha256sum.txt`（明示的に `--checksum-path` で提供可能）と比較。ネットワークが使えないときの最終手段。
- オプション共通で CLI が警告・提案：「チェックサムエラー／証明書エラーが出たら `--cacert-path` か `--insecure` を試してください」。

## 5. テストと TDD の展開
1. `SelfUpdater::needs_update(cur, latest)` の単体テスト（セマンティックな比較）。
2. タグ取得・アーティファクト絞り込みを trait 化した GitHub コネクタのフェイクによる最新タグ検証。
3. ダウンロード＋チェックサムフェイクを作り、一時ファイルの置き換えパスを検証。チェックサム不一致・ダウンロード失敗のケースも追加。
4. atomic `rename` のテスト（`current_exe` モックを使って `ox-v1.0.0` を生成し、失敗時にも `.new` を cleanup）。
5. CLI の `self-update`（`--yes`・`--insecure`・`--offline`）と `rollback` コマンドの統合テスト。

## 6. 実装上の留意点と次のステップ
- コアロジックは UI（ratatui）から分離し、`SelfUpdaterCommand` → `SelfUpdateEvent` などのメッセージベースで協調。
- 進捗や状態は `SelfUpdateStatus`（`Downloading`/`Verifying`/`Updated`/`RollbackReady`/`Failed`）で UI に通知。
- 次の小さなステップ（TDD）
  1. `needs_update` の仕様をまずテストに落とし込み、`Version` 的検出を安定させる。
  2. GitHub 接続と `sha256sums` の取り扱いを trait 化し、フェイクで API エラー/チェックサム不一致を再現。
  3. CLI の既存引数に `self-update rollback`/`--yes`/`--insecure`/`--offline` を追加し、非対話と対話の両方のフローを確認。

## 7. 矛盾と保留事項
- オフライン更新（`--offline`）では GitHub からチェックサムを取れないため、`--checksum-path` でユーザー提供のチェックサムを必須にするか、`--force` で警告を出しながら進めるのか明確化が必要。現状では「チェックサム必須」という原則と矛盾している。
- Release に `sha256sums.txt` が毎回含まれるとは限らないため、チェックサムがないと更新できない仕様はネットワークにアクセスできない環境と両立しない。オプションで `--smart-skip-checksum` などを検討し、セキュリティと利便性のバランスを要検証。

## 削除予定
1. SlackFeedback など TUI 特定の UI 表現（BottomBar への通知）に関する詳細。現行設計では `SelfUpdateStatus` に抽象化するため、UI 固有の単語は削除予定。
2. `sha256sums.txt` を常に要求する記述。存在しない Release に対して更新を拒絶すると、厳しいネットワーク条件での利用に支障をきたすため、オプション化した後に本文から削除する。
3. 「優先度付き Semaphore」など、アップデート処理とは直接関係ない `MetadataFetcher` の項目。別機能文書に移行済みのため、このドキュメントから削除する。
4. `notify`/`ratatui` 固有の記述（UI のアーキテクチャ設計）を減らし、コアロジックに集中する。これらは既存 spec に譜面化済み。
