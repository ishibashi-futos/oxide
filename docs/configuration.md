# Configuration

`ox` の設定は `config.toml` で行います。

フォーマットは単純な `key = value` です。

未設定の項目はデフォルト値が使われます。

`#` 以降はコメントとして扱われます。

## Config file location

優先順位は次の通りです。

1. `OX_CONFIG_HOME` が設定されている場合は `{OX_CONFIG_HOME}/oxide/config.toml`
2. それ以外は `$HOME/.config/oxide/config.toml`

`OX_CONFIG_HOME` も `HOME` もない場合は設定ファイルを読みません。

## Available options

### default_theme

初期テーマを指定します。

値はテーマ名です。

例: `"Glacier Coast"`

未設定ならデフォルトテーマを使用します。

### allow_shell

`/shell` コマンドを有効にします。

`true` を指定すると有効です。

環境変数 `OX_ALLOW_SHELL=true` でも有効になります。

未設定のデフォルトは `false` です。

### allow_opener

ファイルを外部アプリで開く動作を許可します。

`true` を指定すると有効です。

未設定のデフォルトは OS によって変わります。

- Linux: `false`
- Linux 以外: `true`

`allow_opener = false` の場合、`Enter` でファイルを開こうとすると警告を表示します。

## Example

```toml
# ~/.config/oxide/config.toml

default_theme = "Glacier Coast"
allow_shell = false
allow_opener = true
```
