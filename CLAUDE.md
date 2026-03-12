# mdpaste 開発ガイド

## ビルド

```sh
cargo build
```

## コード品質チェック（必須）

コードを変更したら、必ず以下を実行してエラーがないことを確認すること。

### フォーマット

```sh
cargo fmt
```

### Clippy（警告をエラーとして扱う）

```sh
cargo clippy -- -D warnings
```

- `dead_code` 警告: `#[cfg(...)]` ブロック内からしか呼ばれない関数には、同じ `#[cfg(...)]` 属性を付与する
- `manual_split_once` 警告: `splitn(2, x).nth(1)` は `split_once(x).map(|t| t.1)` に書き換える
- その他 Clippy の指摘は原則としてすべて修正する（`#[allow(...)]` での抑制は最後の手段）
