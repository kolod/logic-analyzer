### status: draft
### 機械翻訳による下書きです。日本語を母語とする方による確認が必要です。

### アプリケーション

app-title = Logic Analyzer

### 状態とキャプチャ結果

status-idle = 待機中
status-capturing = キャプチャ中
status-saving = 保存中
status-loading = 読み込み中
status-cropping = データを削除中
status-memory = { $used } / { $limit }
end-reason-completed = キャプチャ完了
end-reason-stopped = ユーザーが停止
end-reason-memory-limit = メモリ制限に到達
end-reason-disconnected = デバイスが切断されました
end-reason-device-error = デバイスエラー
end-reason-samples-lost = サンプルの欠落を伴って完了
end-reason-loaded = ファイルから読み込み
notice-device-removed = 選択したデバイス「{ $text }」は利用できなくなったため、既定のデバイスを選択しました。

### メニュー

menu-file = ファイル
menu-file-new = 新規キャプチャ
menu-file-quit = 終了
menu-capture = キャプチャ
menu-capture-start = 開始
menu-capture-stop = 停止

### ツールバー、ビュー、ダイアログ

toolbar-device = デバイス
toolbar-rate = サンプルレート (Hz)
toolbar-rate-choice = サンプルレート
toolbar-seed = シード
toolbar-seed-random = ランダム
toolbar-profile = 信号
profile-sparse = まばら
profile-normal = 標準
profile-dense = 密
toolbar-start = 開始
toolbar-stop = 停止
status-simulated = シミュレーション
status-incomplete = 不完全
status-unsaved = 未保存の変更
view-no-capture = キャプチャはまだありません。デバイスを選択して「開始」を押してください。
view-empty = キャプチャにサンプルがありません。
view-cannot-draw = 信号を描画できません。
channel-default = チャンネル { $number }
dialog-unsaved-title = 未保存のキャプチャ
dialog-unsaved-body = キャプチャは保存されていません。どうしますか?
dialog-save = 保存
dialog-discard = 破棄
dialog-cancel = キャンセル

### 表示とチャンネル

menu-view = 表示
menu-view-zoom-in = 拡大
menu-view-zoom-out = 縮小
menu-view-fit = 全体を表示
menu-view-start = 先頭へ
menu-view-end = 末尾へ
menu-view-pan-left = 左へ移動
menu-view-pan-right = 右へ移動
menu-view-page-left = 前のページ
menu-view-page-right = 次のページ
menu-capture-follow = 最新データを追従
channel-reset = リセット

### メッセージ

notice-dismiss = 閉じる
notice-dismiss-all = すべて閉じる
notice-log = ログの場所を表示
notice-list = メッセージ
menu-help = ヘルプ
menu-help-log = ログの場所を表示

### マーカーと測定

menu-markers = マーカー
menu-markers-place-single = 単一マーカーを置く
menu-markers-place-pair = ペアマーカーを置く
menu-markers-cancel = 配置をキャンセル
menu-markers-previous = 前のマーカー
menu-markers-next = 次のマーカー
menu-markers-remove = マーカーを削除
menu-markers-remove-all = すべてのマーカーを削除
menu-markers-snap = エッジに吸着
panel-markers = マーカー
marker-single-add = 単一マーカー
marker-pair-add = ペアマーカー
marker-empty = マーカーはありません。上のボタンで追加します。
marker-placing-single = クリックしてマーカーを置きます。Esc で中止します。
marker-placing-pair-first = クリックしてペアの最初の端を置きます。Esc で中止します。
marker-placing-pair-second = クリックして 2 つ目の端を置きます。Esc で中止します。
marker-zoomed-in = エッジを探すために拡大しました。クリックまたは Esc で元の表示に戻ります。
marker-limit = マーカーは最大 64 個です。追加するには 1 つ削除してください。
marker-delete = 削除
marker-comment = コメント
marker-make-pair = ペアにする
marker-join-hint = 2つ目の単独マーカーをクリックします。Esc で取り消し。
marker-action-origin = ここから時間を数える
marker-action-origin-reset = 時間は先頭から数える
marker-action-cut-left = 左側のデータを削除
marker-action-cut-right = 右側のデータを削除
marker-action-scroll = ここへ移動
marker-action-trim = ペアに切り詰める
marker-action-fit = 表示をペアに合わせる
marker-action-split = 2 つの単一マーカーに変換
marker-cut-disabled-running = キャプチャ中は使えません
marker-cut-disabled-nothing = その側に削除するデータはありません
marker-cut-pair-removed = 端が 1 つしか残らないペアを削除しました: { $text }。
dialog-crop-title = データを削除しますか？
dialog-crop-left-body = マーカーの左側のデータを削除します: { $lost }（{ $percent }%）。残りは { $kept } です。この操作は元に戻せません。
dialog-crop-right-body = マーカーの右側のデータを削除します: { $lost }（{ $percent }%）。残りは { $kept } です。この操作は元に戻せません。
dialog-crop-pair-body = ペアの両端の間のデータだけを残します: { $kept }。{ $lost }（{ $percent }%）を削除します。この操作は元に戻せません。
dialog-crop-confirm = 削除
dialog-cropping = データを削除中
marker-colour-0 = 赤
marker-colour-1 = アンバー
marker-colour-2 = 緑
marker-colour-3 = ティール
marker-colour-4 = 青
marker-colour-5 = 紫
marker-colour-6 = ピンク
marker-colour-7 = グレー
panel-capture = キャプチャ
panel-view-capture = キャプチャの設定
panel-view-decoders = デコーダー
panel-view-markers = マーカーと測定
capture-length-seconds = 長さ(秒)
capture-length-samples = サンプル数: { $count }
capture-length-invalid = 長さを秒数で入力してください(例: 2.5)。
capture-length-decimals = 小数点以下は9桁までです。
capture-length-small = このサンプルレートでは1サンプルに満たない長さです。
capture-length-large = キャプチャの上限を超える長さです。
capture-length-no-rate = 先にサンプルレートを設定してください。
capture-channels = 記録するチャンネル
capture-channels-all = すべて
capture-channels-none = なし
capture-no-channel = チャンネルが選ばれていません。開始するには1つ以上選んでください。

### セッションファイル

menu-file-open = 開く…
menu-file-save = 保存
menu-file-save-as = 名前を付けて保存…
menu-file-recent = 最近使ったファイル
menu-file-recent-empty = 最近使ったファイルはありません
menu-file-recent-missing = （見つかりません）
menu-file-recent-remove-missing = 使えない項目を削除
menu-file-recent-clear = リストを消去
dialog-saving = セッションを保存しています…
dialog-opening = セッションを開いています…
dialog-open-title = セッションを開く
dialog-save-title = セッションを保存
dialog-filter-session = ロジックアナライザのセッション
title-untitled = 無題のキャプチャ
notice-view-newer = このファイルの表示設定は新しいバージョンのものなので適用されませんでした。設定はファイル内にそのまま残ります。

### 設定とバージョン情報

menu-file-settings = 設定…
menu-view-scale-up = 表示を拡大
menu-view-scale-down = 表示を縮小
menu-help-about = バージョン情報
settings-title = 設定
settings-unsaved-title = 未保存のキャプチャ
settings-remember-choice = 選択を記憶する
settings-language = 言語
settings-theme = テーマ
theme-system = システムに合わせる
theme-light = ライト
theme-dark = ダーク
theme-high-contrast = ハイコントラスト
settings-scale = 表示倍率
settings-memory = キャプチャのメモリ上限
settings-memory-unit = MiB
settings-memory-note = 次のキャプチャから有効になります。
settings-memory-invalid = { $min } から { $max } までの整数（MiB）を入力してください。
dialog-close = 閉じる
about-title = バージョン情報
about-body = Logic Analyzer { $version }。デジタル信号を記録してタイムライン上に表示します。シミュレーションデバイスで、ハードウェアなしで試せます。
about-license = GNU General Public License バージョン3のもとで公開されている自由ソフトウェアです。

### Decoders: the sample decoder
decoder-sample-clocked-bits-name = クロック付きビット
decoder-sample-clocked-bits-description = クロック線のエッジでデータ線からビットを読み取り、バイトにまとめます。デコーダーにできることを示すサンプルのデコーダーです。
decoder-sample-clocked-bits-signal-clock = クロック
decoder-sample-clocked-bits-signal-data = データ
decoder-sample-clocked-bits-option-clock_edge = クロックエッジ
decoder-sample-clocked-bits-choice-clock_edge-rising = 立ち上がり
decoder-sample-clocked-bits-choice-clock_edge-falling = 立ち下がり
decoder-sample-clocked-bits-option-bit_order = ビット順
decoder-sample-clocked-bits-choice-bit_order-msb = 最上位ビットが先
decoder-sample-clocked-bits-choice-bit_order-lsb = 最下位ビットが先
decoder-sample-clocked-bits-option-bits = 1 バイトのビット数
decoder-sample-clocked-bits-option-idle = アイドル上限（サンプル数）
ann-sample-clocked-bits-byte = バイト { $value }
ann-sample-clocked-bits-bit = ビット { $value }
ann-sample-clocked-bits-incomplete = 不完全なバイト：{ $value } ビット
ann-sample-clocked-bits-gap = ここでサンプルが失われました

### Decoders: I2C
decoder-i2c-name = I2C
decoder-i2c-description = I2Cバス（クロックとデータの2線）を読み取り、各トランザクションのアドレス、バイト、応答を表示します。
decoder-i2c-signal-scl = クロック (SCL)
decoder-i2c-signal-sda = データ (SDA)
decoder-i2c-option-address = アドレスの表記
decoder-i2c-choice-address-addr7 = 7ビットアドレス（方向ビットなし）
decoder-i2c-choice-address-addr8 = 8ビット値（方向ビットあり）
decoder-i2c-option-radix = 進数
decoder-i2c-choice-radix-hex = 16進数
decoder-i2c-choice-radix-dec = 10進数
decoder-i2c-choice-radix-bin = 2進数
ann-i2c-start = スタート
ann-i2c-repeated-start = リピートスタート
ann-i2c-stop = ストップ
ann-i2c-transaction-write = { $value } への書き込み
ann-i2c-transaction-read = { $value } からの読み出し
ann-i2c-address-write = アドレス { $value }（書き込み）
ann-i2c-address-read = アドレス { $value }（読み出し）
ann-i2c-data = データ { $value }
ann-i2c-ack = ACK
ann-i2c-nak = NAK
ann-i2c-incomplete = 未完了のトランザクション
ann-i2c-error-byte-cut = { $value } ビット後にバイトが途切れました
ann-i2c-error-address-nak = アドレス { $value } に応答がありません
ann-i2c-error-data-nak = データ { $value } に応答がありません
ann-i2c-error-empty = 空のトランザクション：アドレスがありません
ann-i2c-error-gap = ここでサンプルが失われました

### Decoders: menu, panel and the selected annotation
menu-decoders = デコーダー
menu-decoders-add = デコーダーを追加
menu-decoders-previous = 前の注釈
menu-decoders-next = 次の注釈
menu-decoders-goto = 選択した注釈へ移動
panel-decoders = デコーダー
decoders-add = デコーダーを追加
decoders-empty = デコーダーはまだありません。追加すると、信号の内容が分かります。
decoders-need-capture = 先に信号をキャプチャするか開いてください。
decoder-state-waiting = 待機中
decoder-state-running = 実行中
decoder-state-finished = 完了
decoder-state-cancelled = キャンセル済み
decoder-state-failed = 失敗
decoder-needs-channels = デコードを開始するチャンネルを選んでください。
decoder-channel-none = （なし）
decoder-lane-show = レーンを表示
decoder-annotations = 注釈数: { $number }
decoder-incomplete = 結果は不完全です
decoder-selected = 選択中の注釈
decoder-detail-length = 長さ: { $value }
decoder-detail-error = 信号の問題を示しています
decoder-detail-position = 位置: { $first } から { $second }

### Decoders: the simulation of signals that are valid for a decoder
decoder-generator-errors = シミュレートした信号にわざとエラーを入れる
decoder-generator-rate = おおよそこの数に 1 バイトが途中で切れます

### Decoders: running, cancelling and the memory limit
decoder-run = もう一度実行
decoder-cancel = キャンセル
settings-decoder-memory = デコーダーの結果のメモリ上限
settings-decoder-memory-note = すべてのデコーダーで共有します。次の実行から有効です。

### Decoders: stacking
decoder-source = 入力元
decoder-source-capture = キャプチャ
dialog-remove-decoder-title = デコーダーを削除
dialog-remove-decoder-body = このデコーダーから他のデコーダーが読み取っています。削除するとそれらも削除されます。キャプチャには影響しません。
dialog-remove-decoder-confirm = すべて削除

### Decoders: saving and opening
notice-decoders-newer = このファイルには、より新しいバージョンのアプリケーションのデコーダーが含まれています。そのまま保持され表示されませんが、キャプチャは通常どおり開きます。
notice-layout-newer = このファイルには、新しいバージョンのアプリケーションが保存した信号の高さが含まれています。そのまま保持され表示されません。取り込みは自動の高さで開きます。
settings-save-decoder-results = デコード結果をセッションと一緒に保存する
settings-save-decoder-results-note = ファイルが大きくなります。保存しない場合は、ファイルを開くときにデコーダーが再実行されます。
