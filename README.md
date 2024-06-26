# minecraft-server-wrapper

マインクラフト統合版のサーバーのバックアップを自動で取るためのプログラムです。

## インストール

1. Rustをインストールします。
1. 以下のコマンドを実行します。

```sh
cargo install --git https://github.com/mousecrusher2/minecraft-server-wrapper.git
```

## 使い方

以下のコマンドを実行してサーバーを起動します。

```sh
minecraft-server-wrapper --server-path <サーバーのパス> --backup-folder <バックアップを保存するディレクトリのパス> --backup-interval <バックアップを取る間隔>　--backup-count <バックアップを保持する数>　-- <サーバーの起動オプション>
```

`--server-path` はサーバーのパスを指定します。指定しなかったっ場合は`./bedrock_server`が使われます。  
`--backup-folder` はバックアップを保存するディレクトリのパスを指定します。指定しなかった場合は`./backups`が使われます。  
`--backup-interval` はバックアップを取る間隔を指定します。指定しなかった場合はバックアップは取られません。`1h 30m` `30m` `10s`のように指定します。詳しくは[chronoのドキュメント](https://docs.rs/humantime/latest/humantime/fn.parse_duration.html)を参照してください。  
`--backup-count` はバックアップを保持する数を指定します。指定しなかった場合は直近のバックアップのみが残ります。  
`--` 以降にはサーバーの起動オプションを指定します。例えば、`-- --port 19132`のように指定します。

起動後はバックアップが指定した間隔で取られます。  
手動でバックアップを取る場合は`!backup`と入力してください。  
`!`で始まるコマンドは全てこのプログラムが処理し、サーバーには送信されません。`!`で始まらないコマンドは全てサーバーに送信されます。  
バックアップは`<バックアップを保存するディレクトリのパス>/<ワールド名>/<日時>/`に保存されます。  
`save hold` `save resume` `save query`などのコマンドは使用しないでください。バックアップ処理が正常に行われなくなる可能性があります。  
サーバーを終了する場合は`stop`と入力してください。
