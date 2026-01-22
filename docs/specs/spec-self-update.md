# セルフアップデート (self-update) 機能設計

## 概要
`ox` を GitHub Releases の最新版（または指定バージョン）へ安全に更新する機能を提供する。
CI/CD と連携し、ユーザーが常に最新の改善を享受できるようにする。
最小限の操作でローカルのバージョンを検出し、タグ・Release・アーティファクト・チェックサムを順に確認しながら安全に置き換える。

## 1. ユーザー体験 (UX)

### 基本フロー
- `ox self-update`
  - 最新版を確認し、更新があれば `v0.1.0 -> v0.2.0` のように差分を表示。
  - `Do you want to update? [y/N]` で確認。
  - 完了後、再起動を促すか、そのまま終了。

### オプション操作
- `ox self-update --tag v1.0.0`: 特定バージョンへの変更（アップグレード/ダウングレード）。
- `ox self-update --prerelease`: プレリリース版も含めて最新を探す。
- `ox self-update rollback`: 直前のバックアップから復元（オフライン可）。
- `ox self-update --yes`: 確認プロンプトをスキップ（CI/スクリプト用）。

## 2. バージョン解決と判定ロジック

### バージョン情報の取得
- Current:
   - CI ビルド時は環境変数 `OX_BUILD_VERSION` (GitHub Ref Name) を埋め込み、タグ名 (`v1.0.0` 等) をそのままバージョンとする。
   - ローカル開発時は `Cargo.toml` のバージョン (`CARGO_PKG_VERSION`) をフォールバックとして使用する。
   - これにより、ローカルでも `OX_BUILD_VERSION=v0.0.1 cargo run` とすることで任意のバージョンを偽装し、アップデート通知のテストが可能になる。

### ターゲット決定 (ReleaseFinder)
- GitHub API (`/repos/ishibashi-futos/oxide/releases`) を使用。
- デフォルト: `prerelease: false` の中で `semver` 最大のもの。
- `--prerelease`: `prerelease: true` も含めた中で `semver` 最大のもの。
- `--tag <TAG>`: 指定タグと完全一致するリリース。

### 更新判定
- `Target > Current`: 更新 (Update)。
- `Target < Current`: ダウングレード。警告を表示して続行可能。
- `Target == Current`: 何もしない（`--force` で強制可）。

## 3. 更新プロセス (Core Flow)

### 3.1 ダウンロードと検証
- 最新タグに対応する Release （`assets`）から `ox-{target}-{version}` の命名ルールにマッチする行を選ぶ。対象は `OS`/`ARCH`/`target triple` で決定し、見つからなければエラー。
- `reqwest` 等の HTTP クライアントでストリームを一時ファイル（`tempdir`）に書き込む。進捗表示は optional で `SelfUpdateStatus::Downloading` などを TUI に送る。
- `sha256sums.txt` を同じ Release から取得し、対象アーティファクトの行を抽出。`sha2` でローカルファイルのハッシュを計算して一致を確認。
- `sha256sums` が存在しない Release では警告し、`--force-checksum` のようなオプションか `--insecure-checksum` （危険）を使うまで継続しない。

### 3.2 原子的な置き換え (Atomic Replacement)

- `current_exe()` を得て、そのあるディレクトリに `ox-v<current>` を作成し、`std::fs::copy` か `rename` で退避。
- ダウンロード済みバイナリを `ox.new` などに移し、`std::fs::rename` で `ox` と入れ替える。
- 置き換え後は権限を維持し（`chmod +x`）、一時ファイルを削除。失敗したら元バイナリとバックアップを継続させる。
- `SelfUpdateStatus::Updated(old, new)` を TUI に送る。

#### Windows 特有の処理 (Move-away方式)
Windows では実行中のバイナリを上書き・削除できないため、以下の手順を採用する。

1. **退避**: 実行中の `ox.exe` を `ox-old.exe` (またはバージョン付きバックアップ名) にリネームする。実行中でもリネームは可能。
2. **配置**: 新しいバイナリ `ox.new` を `ox.exe` にリネームする。

## 4. 安全性とリカバリ

### ロールバック
- `ox self-update rollback` を CLI に用意し、実行可能ファイルと同じディレクトリ内の `ox-v*` を列挙してユーザーに選ばせる。
- 選択したローカルファイルを `ox` にリネーム（またはコピー）して復元する。
- ネットワーク通信は行わず、手元のバックアップのみを使用する緊急復旧手段とする。
- 特定の過去バージョンを GitHub から再取得したい場合は `ox self-update --tag <version>` を使用するよう案内する。

### 証明書／ネットワーク制限対応
- ネットワーク固有の`TLS中間証明書`等の影響を考慮して、`reqwest::ClientBuilder` に CA ストア追加や検証無視のオプションを組み込む。
- CLI オプション
  - `--cacert-path <path>` / `OX_SELF_UPDATE_CACERT`: `.cer`/`.pem` 形式の証明書を持ち込み、`ClientBuilder::add_root_certificate` で信頼する。
  - `--insecure`: `danger_accept_invalid_certs(true)` を有効化。出力で明示的に警告し、自動化向けには `--insecure` の明示を必須とする。
  - `--offline --binary-path <path>`: すでにダウンロード済みのバイナリを指定し、チェックサムはローカルの `sha256sum.txt`（明示的に `--checksum-path` で提供可能）と比較。ネットワークが使えないときの最終手段。
- オプション共通で CLI が警告・提案：「チェックサムエラー／証明書エラーが出たら `--cacert-path` か `--insecure` を試してください」。

## 5. 開発・テスト計画 (TDD)

1. バージョン比較: `needs_update` ロジックの単体テスト。
2. ReleaseFinder: GitHub API レスポンスのモックを用いたバージョン選択ロジック（`--tag`, `--prerelease`）のテスト。
3. ダウンロード・検証: チェックサム不一致、アセット欠損のシミュレーション。
4. 置換・ロールバック: ファイルシステム操作のモックまたは一時ディレクトリでの統合テスト。
5. CLI統合: `self-update`（`--yes`・`--insecure`・`--offline`）と `rollback` コマンドの引数パースとフロー確認。

## 6. 実装状況

### できていること
- `ox self-update` の基本フローが動く。
- `--yes` で確認をスキップできる。
- `--tag <TAG>` で指定タグを選べる。
- `--prerelease` を付けたときだけプレリリースを含めて探す。
- GitHub Releases API から `assets[].digest` を使って検証する。
- `tar.gz`/`tgz` のアーカイブを展開し、中の `ox` を置き換える。
- 置き換え前にバックアップを作り、`rollback` で復元できる。

### TODO

- [x] self-updateの処理をにまとめ、TUIのUI/Coreとの境界を明確にする。
- [ ] `TLS/CA` を実装する。
- [x] `--insecure` を実装する。
- [ ] `--offline` を実装する。
- [x] Windowsの置換手順を明確にし、必要なら専用処理を入れる。 (Move-away方式を採用)
- [x] Windowsの場合の置換処理の実装
- [ ] `--force` などの強制更新・チェックサム例外の設計を詰める。

## 7. self-update処理の構成分離

- `src/self_update/` 以下にコア処理をまとめることで TUI に依存しない構造を目指す。
- モジュール例として `config.rs` で `SelfUpdateConfig` を定義し、`service.rs` でライフサイクルを orchestrate
 する。

- `release.rs` で `ReleaseFinder` を置き、GitHub API、ターゲット選定、`needs_update` 判定を分離してテストしやすくする。
- `download.rs` はアセット選別、ストリーミング、`sha256` 検証、`SelfUpdateStatus::Downloading` を扱い、進捗通知をトリガーする。
- `replace.rs` には `AtomicReplacer` とロールバックロジックを集約し、成功・失敗のステータスを返す。
- `traits.rs` などで HTTP クライアント、ファイル操作をトレイトとして抽象化し、テスト時にはモックと差し替える。

- CLI では `SelfUpdateConfig` を構築し、`service::SelfUpdateService::run(config, notifier)` を呼ぶだけにする。
- UI への通知は `SelfUpdateStatus` を受け取り、TUI コマンドや確認プロンプトの材料に変換する。
- CLI はコアに対して状態通知と結果だけを受け取り、ユーザーへの説明、再起動案内、エラー表示を行う。

- これによりコアは `SelfUpdateError` などを返す責務だけに集中でき、CLI の復帰処理や表示とは独立する。
- テストはコアの各モジュールを単体でモック化し、CLI 層は `SelfUpdateService` をダミー実装で置き換えてフロー検証を行う。
