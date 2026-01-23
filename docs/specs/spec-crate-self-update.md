# セルフアップデート共通クレート設計 - draft

## 目的
- `ox` の `src/self_update` を独立したクレートに切り出し、他の CLI バイナリでも再利用できる汎用インターフェースを提供する。
- GitHub Releases を使ったバイナリ配布に対応し、ダウンロード／検証／差し替えの責務を UI 層から分離する。
- GitHub のプライベートリリースや OS/アーキテクチャ差を考慮した設計とし、トークンなど追加情報を外部から注入可能にする。

## 1. コア要件
1. GitHub の Releases API からリリース一覧を取得し、更新対象を判定する。
2. リリースアセットの名前を OS・アーキテクチャ（target triple）で絞り込み、該当バイナリをダウンロードする。
3. ダウンロード後に SHA256 などのチェックサム検証を行い、改ざんを検出したら処理を中止する。必要に応じて強制実行オプションも提供する。
4. ダウンロード済みバイナリを現在の実行ファイルと原子的に置き換え（Windows では move-away）、必ずバックアップとロールバック手段を持つ。
5. 処理の進行状況や結果（更新があった・なかった・失敗した）を呼び出し元に通知できるステータス列挙型とし、UI 層はそれを UI/ログに翻訳するのみとする。
6. プライベートリリースに対応するため、必要に応じて GitHub API にトークン（`Authorization: Bearer`）を付与する。
7. TLS 証明書や CA バンドルのカスタマイズ、自己署名サーバーへの対応も柔軟にできる構造にする。
8. Test-Driven Development を踏まえ、トレイト分離でインターフェースを抽象化し単体テスト可能にする。

## 2. 公開 API の骨子
- `SelfUpdateConfig`
  - `repo: String`（`owner/name`）
  - `allow_prerelease: bool`
  - `allow_insecure: bool`
  - `auth_token: Option<String>`（GitHub API 認証に使う）
  - `custom_ca: Vec<PathBuf>` など TLS/CA を注入するオプション（ファイルパスや DER 文字列）
  - デフォルトユーザーエージェントと、必要なら `github_api_base: Url` などで GitHub Enterprise 対応を用意する。
- `VersionEnv` トレイト（`get(var: &str) -> Option<String>`）として `OX_BUILD_VERSION` などビルド時情報を外部注入可能にする。
- `SelfUpdateService` / `SelfUpdatePlan`
  - `plan_latest(config, env, cargo_version)`
  - `plan_tag(config, env, cargo_version, tag)`
  - `download_asset(asset, config)`
  - `replace_current(downloaded, target_tag)`
  - `list_backups()`
  - `rollback(backup)`
- `SelfUpdateStatus`（`Downloading`, `Verifying`, `BackupCreated(PathBuf)`, `Updated(PathBuf, PathBuf)`, `Rollback(PathBuf)` など）を定義し、UI へ通知するイベントを外部に委ねる。
- `SelfUpdateError` でエラー種別を列挙（`Network`, `ChecksumMismatch`, `ReplaceFailed`, `AuthenticationFailed`, `NoMatchingAsset` など）。

## 3. モジュール構成と責務
1. `config`：`SelfUpdateConfig` とそれに付随するビルダー。`auth_token` や `custom_headers` をまとめ、条件付き動作（例: `allow_insecure`）を記録する。
2. `traits`：HTTPやファイル操作、`VersionEnv` や `Clock` など、テスト時に差し替えるためのトレイトを含む。
3. `http`：`HttpClientBuilder`（`allow_insecure`, `custom_ca`, `auth_token` など）を `ureq` / `reqwest` などの実装で提供。将来的には `async` 版を追加しやすいよう `AgentProvider` を抽象化。
4. `release`：リリース一覧の取得／解析、最新リリース選択、タグ指定、`semver` 比較ロジック、アセット名のマッチングなど。private repo 対応のため `Authorization` ヘッダーの挿入と `api_base` の差し替えもこの層で処理。
アセット名のマッチングは、CI の命名規則に合わせて `target triple` や OS/ARCH/バイナリ名を受け取って文字列を返す関数を `SelfUpdateConfig` などから渡して制御できるようにし、`ox-aarch64-apple-darwin.tar.gz` 以外の形式（例: `oxide-aarch64-darwin.tar.gz`, `ox-${version}-${os}`）にも柔軟に対応できる。
5. `download`：アセットをストリーミング保存して `SelfUpdateStatus::Downloading` を通知。チェックサム検証（リリースに `digest` がない場合は `sha256sums.txt` を探す）と失敗時のリトライ制御（`--force-checksum` を想定）を含む。
6. `replace`：アトミックな差し替え処理、バックアップ作成、Windows の move-away。エラーが発生した場合には `SelfUpdateError::ReplaceFailed` で明示的に詳細を伝える。
7. `service`：上記をオーケストレーションし、`SelfUpdatePlan` を返す。`plan_latest` では `allow_prerelease` や `force` を考慮して `UpdateDecision` を出し、エラーを UI に伝える。

## 4. GitHub Release とのやり取り
- `fetch_releases` は設定された `repo` と `api_base` を組み合わせて `GET /repos/{repo}/releases` を呼び出す。`auth_token` が指定されていれば `Authorization: Bearer {token}` を付与。
- `allow_prerelease` フラグが `false` なら `draft` と `prerelease` をフィルタ。private release の場合でも `tag_name` が期待を満たせば利用可能。
- Asset 選択は OS/ARCH triple と `asset_prefix`（例: `ox-linux`）でスコアリング。ヒットしない場合は `SelfUpdateError::AssetNotFound` を返し、`plan` は UI に明示的に表示させる。
- Private release では `digest` フィールドがなくても `sha256sums.txt` を同じ Release から取得し、対象バイナリ名をキーにチェックサムを照合。

## 5. ダウンロードと検証
- `DownloadTarget` に `GitHubAsset` と `PathBuf`（temp file）を持たせて処理する。
- ストリーミング保存中に `SelfUpdateStatus::Downloading(url, progress?)` を通知するため、`Notifier` トレイトを導入。
- 取得したファイルに対して `sha256` を計算し、リリース `digest` または `sha256sums.txt` から得た値と比較。失敗時は `SelfUpdateError::ChecksumMismatch`。
- `allow_insecure` が `true` の場合でもチェックサム検証は行うが、`--insecure` により TLS 検証だけをスキップする。

## 6. 置き換えとロールバック
- `current_exe()` を使って現行バイナリを特定し、同じディレクトリに `ox-old-{timestamp}.exe` などバックアップを作成。
- 新しいバイナリを `ox.new` に移してから `rename` で切り替え。Unix 系は `std::fs::rename`、Windows では `DeleteFile` できないため `move-away` を採用。
- `replace` 関数は `SelfUpdateStatus::BackupCreated(PathBuf)` などで通知。`rollback` は最新バックアップを列挙・選択して `rename` で復元する。
- `replace_current` は `TempDir` を使って作業するため、失敗時には元バイナリを復旧させる。

## 7. エクステンションポイント（TUI/CLI からの使い方）
- CLI は `SelfUpdateConfig::builder().repo(...).auth_token(...)` などで `config` を構築し、`SelfUpdateService::plan_latest` → `download_asset` → `replace_current` をシーケンスで呼び出す。
- 進捗や結果は `SelfUpdateStatusNotifier` トレイトを実装したコンポーネントが購読し、UI/ログへ翻訳。
- `SelfUpdatePlan` には `decision: UpdateDecision` を含めることで CLI は `yes` フラグや `force` を評価できる。
- `SelfUpdatePlan::asset_for_target` を使って、ユーザーが明示的に選んだアセットをダウンロードできるようにする。

## 8. テスト戦略（TDD）
1. `ReleaseTarget` を対象にした `parse_version_tag`/`decide_update` の単体テスト。
2. `fetch_releases` で `ureq::Agent` をモックし、`auth_token`・`api_base` のヘッダー追加や `allow_prerelease` のフィルタを検証。
3. `select_target_asset` と `DownloadTarget` で OS triple によるフィルタとチェックサム比較をモックしたストリームに対してテスト。
4. `replace` の `rename`/`backup`/`rollback` を `tempdir` を使った統合テストで確認。
5. `SelfUpdateService` の `plan_latest/plan_tag` を `VersionEnv` モックと `HttpClient` モックで TDD。
6. エンドツーエンドでは `allow_insecure`, `allow_prerelease`, `auth_token` の組み合わせ、`SelfUpdateStatus` の通知パターンを確認する。

## 9. ドキュメントとパッケージング
- README に API 呼び出し例、`Cargo.toml` での `self_update_core` の使い方、GitHub トークンの設定例（`GITHUB_TOKEN`、`OX_SELF_UPDATE_TOKEN`）、`allow_insecure` を使う際の注意点を記載。
- `examples/` に `plan_and_replace.rs` などのデモを置き、GitHub Release を模したモックサーバーとの対話を示す。
- CI で `cargo test` に加えて `cargo fmt`/`clippy` を通す。

以上を踏まえ、`ox` 本体はこのクレートを依存として組み込み、UI から呼び出すだけの構造へと整理する。
