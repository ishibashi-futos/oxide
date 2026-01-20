# タブごとのカラーテーマ

## 要約
タブごとに色テーマを持たせる。

色はタブの状態と文脈の識別を助ける。

## 目的
タブ切替時の迷いを減らす。

現在タブの状態を即時に把握できるようにする。

## ユーザー体験（概要）

- 色でタブを区別したい。
- `/color` で現在タブのテーマを切り替えたい。
- 色の意味をすばやく理解したい。

## 必須要件

### データ構造

- `ColorThemeId` は列挙型とする。
- `ColorTheme` は `base`, `primary`, `secondary`, `semantic` (success/warn/error/info), `grayscale` (low/high) を含む。
- タブは `ColorPreference { tab_id, theme_id: ColorThemeId }` を保持する。

### テーマ割当

- `ColorThemeId` は最低 5 種類を持つ。
- 新規タブは `ThemeRotation` で順番にテーマを割り当てる。
- `config.toml` の `default_theme` で初期テーマを上書きできる。

### メッセージ連携

- Core は `TabColorChanged { tab_id, theme: ColorTheme }` を UI に送る。
- UI はそれを受けて `TabBorder`, `Selection`, `Metadata` の色を更新する。

### `/color` コマンド

- `/color {theme name}` で現在タブにテーマを割り当てる。
- UI は `SlashCandidates { items }` でテーマ名の候補を提示する。
- 候補には `description` で semantic の役割を添える。
- `/color` 実行中は `CommandContext::Tab(tab_id)` を強制する。
- 存在しないテーマ名は `SlashFeedback` で `status=Error` を返す。
- 正常な `theme_id` を受けたら Core 側で `tab.set_theme(theme_id)` を呼ぶ。
- 成功時は `SlashFeedback` で `text = format!("theme: {}", theme.name)` を返す。

### UI 反映ルール

- アクティブなタブのボーダーはテーマの `base` を使う。
- 非アクティブタブのボーダーは同一テーマのグレースケール低彩度を使う。
- 選択・フォーカス状態の行は `primary` を使う。
- `secondary` は補助的な領域に使う。
- `semantic` は結果表示やエラー表示に使う。
- `grayscale` は背景、分割線、メタデータ表示に使う。

## テーマ定義

| テーマ名 | ベース (タブボーダー) | プライマリ | セカンダリ | 成功 | 警告 | エラー | 情報 | グレースケール (low/high) | 備考 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Glacier Coast | `#4E6C92` | `#8AB4F8` | `#6AA0E7` | `#5EC38F` | `#F4B04C` | `#E38A90` | `#5DD0FF` | `#1F242A` / `#DDE4ED` | 青系のクールな透明感 |
| Night Harbor | `#3E5A63` | `#7ED0D9` | `#5CA9B4` | `#3CC48C` | `#F2A666` | `#D67278` | `#5FB7FF` | `#21262A` / `#CED7E0` | 緑寄りティールで落ち着いた海の印象 |
| Slate Dawn | `#4B5161` | `#9AA2B7` | `#7C86A3` | `#6BC8A4` | `#F0AD6C` | `#DA7D7D` | `#76B0FF` | `#20252E` / `#D3D7DF` | 紫がかったスレート調で静かな朝 |
| Aurora Drift | `#4E5F4F` | `#A4D8C3` | `#7EBCA8` | `#50C073` | `#F7BF5C` | `#E0827A` | `#8FD2FF` | `#1E2420` / `#D7E3D7` | コールドグリーンに白さを添えた北欧的 palette |
| Deep Forest | `#3F4F46` | `#8AC6A5` | `#65A08A` | `#48B87B` | `#F0B15A` | `#DD7B80` | `#7DAFFF` | `#1C221C` / `#CCD7D3` | 緑深い森林を想起させる低彩度グリーン |

## カラー用途とコード参照

| 色種 | 主な用途 | 対応コード位置 |
| --- | --- | --- |
| `base` | タブとメインペインの枠線/ブロックのボーダー。`src/ui/main_pane.rs:31-43` の `Block::default().borders(Borders::ALL).title(title)` に `Style::fg(theme.base)` を注入し、`frame.render_widget` 前に基調を設定。 |
| `primary` | カーソル選択・フォーカス・検索中のハイライト。`src/ui/main_pane.rs:62-100` で `highlight_style`/`highlight_symbol` に `theme.primary` を反映し、`List` の `highlight_style` とシンボルに使う。 |
| `secondary` | 複数ヒット時の補助表示や非アクティブ領域。`secondary_match_style`（`src/ui/main_pane.rs:72-75`）や `format_tabs` の `TabSummary` 表示（`src/ui/bottom_bar.rs:121-133`）で `theme.secondary` を適用する。 |
| `semantic` | `SlashFeedback.status`（`src/app.rs:518-568`）に準じる成功/エラーなどのバッジ。`src/ui/bottom_bar.rs:13-71` の `build_bottom_bar` 中で `SlashFeedback` テキストに `Style::fg(theme.semantic.success)` / `theme.semantic.error` を付与し、必要があれば `SlashCandidates` などにも派生させる。 |
| `grayscale` | ボトムバー全体・スラッシュバー・背景。`src/ui/bottom_bar.rs:31-40` の `render_slash_bar` で `Color::DarkGray` を `theme.grayscale.low/high` に置き換え、metadata 行や分割線にも `high` を割り当てる。 |

## TDD ステップ

1. `ColorThemeId` と `ColorTheme` を定義し、パレットのプロパティを返す単体テストを書く。
2. `tabs::Tab` にテーマ設定を組み込み、`set_theme` / `current_theme` の振る舞いをテストする。
3. `/color` コマンドで `SlashFeedback` を返すモックを作り、存在しないテーマや成功時のメッセージを検証する。
4. UI の `TabBar` に `TabColorChanged` を受け取る仕組みを追加し、レンダリングカラーが期待どおりになるか確認する。
