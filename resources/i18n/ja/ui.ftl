### status: draft
### 機械翻訳による下書きです。日本語を母語とする方による確認が必要です。

### アプリケーション

app-title = Logic Analyzer

### 状態とキャプチャ結果

status-idle = 待機中
status-capturing = キャプチャ中
status-saving = 保存中
status-loading = 読み込み中
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
toolbar-length = 長さ (サンプル数)
toolbar-seed = シード
toolbar-seed-random = ランダム
toolbar-profile = 信号
profile-sparse = まばら
profile-normal = 標準
profile-dense = 密
toolbar-realtime = リアルタイム
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
panel-channels = チャンネル
channel-move-up = 上へ
channel-move-down = 下へ
channel-colour = 色
channel-reset = チャンネルをリセット

### メッセージ

notice-dismiss = 閉じる
notice-dismiss-all = すべて閉じる
notice-log = ログの場所を表示
notice-list = メッセージ
menu-help = ヘルプ
menu-help-log = ログの場所を表示

### マーカーと測定

menu-markers = マーカー
menu-markers-place = マーカーを置く
menu-markers-previous = 前のマーカー
menu-markers-next = 次のマーカー
menu-markers-remove = マーカーを削除
menu-markers-remove-all = すべてのマーカーを削除
menu-markers-snap = エッジに吸着
panel-markers = マーカー
marker-default = マーカー { $number }
marker-go = マーカーへ移動
marker-remove = 削除
marker-measure-first = 測定の始点
marker-measure-second = 測定の終点
measure-distance = 間隔
measure-frequency = 周波数
measure-pick = 2つのマーカーを選ぶと、その間の時間を測定できます。

### セッションファイル

menu-file-open = 開く…
menu-file-save = 保存
menu-file-save-as = 名前を付けて保存…
menu-file-recent = 最近使ったファイル
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
decoder-remove = デコーダーを削除
decoder-annotations = 注釈数: { $number }
decoder-incomplete = 結果は不完全です
decoder-selected = 選択中の注釈
decoder-detail-length = 長さ: { $value }
decoder-detail-error = 信号の問題を示しています
decoder-detail-position = 位置: { $first } から { $second }

### Decoders: the simulation of signals that are valid for a decoder
decoder-generator-errors = シミュレートした信号にわざとエラーを入れる
decoder-generator-rate = おおよそこの数に 1 バイトが途中で切れます
