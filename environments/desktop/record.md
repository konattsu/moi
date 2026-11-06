# Arch Linux デスクトップ環境構築記録

2026年10月22日｜Hyprland / Wayland / UWSM / greetd

## 1. 概要

既存のArch Linux環境に、Hyprlandを中心としたWaylandデスクトップ環境を構築した。

GNOMEやKDE Plasmaのような統合デスクトップ環境は導入せず、コンポジター、音声、認証、通知、ファイル管理などを独立したコンポーネントとして組み合わせる構成を採用した。

今回の目的は、GUI環境として必要な機能を一通り成立させること。外観、テーマ、配色、細かな操作性などのカスタマイズは今後行う。

## 2. システム構成

| 分野                       | 採用技術                                         |
| ------------------------ | -------------------------------------------- |
| OS                       | Arch Linux                                   |
| Display Protocol         | Wayland / XWayland                           |
| Compositor / WM          | Hyprland                                     |
| Terminal                 | kitty                                        |
| Application Launcher     | hyprlauncher                                 |
| Status Bar               | Waybar                                       |
| Session Manager          | UWSM                                         |
| Display Manager          | greetd                                       |
| Greeter                  | tuigreet                                     |
| Audio Server             | PipeWire / pipewire-pulse                    |
| Audio Session Manager    | WirePlumber                                  |
| Network                  | NetworkManager                               |
| Desktop Portal           | xdg-desktop-portal-hyprland / gtk            |
| Privilege Authentication | hyprpolkitagent                              |
| Lock / Idle              | hyprlock / hypridle                          |
| Notification             | swaync                                       |
| Wallpaper                | hyprpaper                                    |
| File Manager             | Thunar                                       |
| File Management Support  | Tumbler / GVfs / udisks2 / ffmpegthumbnailer |
| Clipboard                | wl-clipboard / cliphist / fuzzel             |
| Screenshot               | grim / slurp                                 |
| Brightness               | brightnessctl                                |
| Real-time Scheduling     | RTKit                                        |
| Fonts                    | Noto / Noto CJK / Noto Emoji / DejaVu        |

GUIの起動経路は次の構成を目指した。

`systemd → greetd → tuigreet → UWSM → Hyprland`

ログイン後、Hyprlandを中心にWaybar、PipeWire、Portal、通知などが動作する。

## 3. 構築経過

### 第1段階：最小GUI

Hyprland、XWayland、kitty、hyprlauncher、Waybar、Mesa、フォントを導入した。

TTYから`start-hyprland`で起動し、ターミナル操作、アプリケーション起動、ウィンドウ管理を確認。HyprlandのLua設定ファイルを利用してキーバインドとWaybarの自動起動を設定した。

### 第2段階：デスクトップ基盤

PipeWire、WirePlumber、NetworkManager、XDG Desktop Portal、Polkit認証エージェント、hyprlock、hypridleを導入した。

ネットワークはNetworkManagerで管理し、Wi-Fi接続を確認。音声は当初Dummy Outputしか表示されなかったが、原因を調査して解決した。

Portalについては、起動ログにRealtimeKit関連の警告があったため、RTKitを追加導入した。

### 第3段階：日常利用機能

通知、壁紙、ファイル管理、USBストレージ操作、クリップボード履歴、スクリーンショット、輝度調整のためのコンポーネントを追加した。

アプリケーションの常駐はHyprlandの起動イベント、ファイル管理やストレージ関連の機能は必要時に起動するサービスを利用する方針とした。

### 追加段階：セッション・ログイン管理

UWSMによるsystemdユーザーセッション管理、greetd + tuigreetによるログイン画面、Hyprland側でのキーボード配列設定を採用する方針を決定した。

ただし、この追加段階は手順の提示と設計判断までが会話上で確認できており、実際の導入完了・動作確認は未報告である。

## 4. Architecture Decision Records（ADR）

以下では、採用した設計とその理由、代替案、トレードオフを記録する。

### ADR-001：統合デスクトップ環境を使用しない

採用

Decision: GNOME / KDE Plasmaを導入せず、Hyprlandを中心に必要なコンポーネントを個別に構成する。

Rationale: 各機能の責務と依存関係を明確にし、必要な機能だけを導入できる環境を優先した。また、外観や操作性を後から自由に変更できる。

Trade-off: 初期設定やトラブルシューティングを自分で行う必要があり、統合デスクトップ環境と比較して構成管理の負担が大きい。

### ADR-002：段階的な構築と検証

採用

Decision: GUI、デスクトップ基盤、日常利用機能、セッション管理の順に導入する。

Rationale: 依存関係を整理し、問題発生時に原因を特定しやすくするため。特に最初はDisplay Managerを導入せず、TTYから直接Hyprlandを起動することで、ログイン管理とコンポジターの問題を分離した。

Trade-off: 完成までの手順は増えるが、障害切り分けと構成理解を優先した。

### ADR-003：Waylandを基本とし、XWaylandで互換性を確保する

採用

Decision: Waylandを基本プロトコルとし、X11アプリケーションにはXWaylandを使用する。

Rationale: Hyprlandを中心としたWaylandネイティブ環境を構築しつつ、既存のX11アプリケーションとの互換性を確保するため。

Trade-off: 一部のX11アプリケーションではWaylandネイティブアプリケーションと挙動が異なる可能性がある。

### ADR-004：音声基盤にPipeWireを採用する

採用

Decision: PipeWire + WirePlumberを採用し、pipewire-pulseによってPulseAudio互換性を確保する。

Rationale: デスクトップ音声と画面共有などのメディア処理を統一的に扱えるため。RTKitも導入し、リアルタイム性が必要な音声処理を支援する。

Trade-off: カーネル、ALSA、SOFファームウェア、PipeWire、WirePlumberという複数の層を理解する必要がある。

### ADR-005：XDG Desktop Portalを採用する

採用

Decision: `xdg-desktop-portal-hyprland`と`xdg-desktop-portal-gtk`を併用する。

Rationale: アプリケーションに対し、画面共有やファイル選択などのデスクトップ機能を共通インターフェースで提供するため。Hyprland固有の機能と汎用的なGTK機能を分担させる。

Trade-off: Portal本体、バックエンド、D-Bus、セッション環境変数の整合性が必要になる。

### ADR-006：UWSMによるセッション管理

採用方針決定・導入未確認

Decision: UWSMを導入し、Hyprlandセッションをsystemdユーザーサービスと統合する。

Rationale: セッションのライフサイクル、環境変数、バックグラウンドサービスを統一的に管理するため。将来的には常駐コンポーネントもsystemdユーザーサービスに整理する。

Trade-off: 従来のHyprland起動イベントによる管理から移行する必要があり、サービスの重複起動や終了処理に注意が必要。

### ADR-007：greetdとgreeterを分離する

採用方針決定・導入未確認

Decision: Display Managerにgreetd、初期greeterにtuigreetを採用する。

Rationale: ログイン認証とUIを分離し、今後GUIログイン画面に変更する場合もDisplay Managerを交換せずに済む構成とする。

Alternatives: SDDM、GDM、LightDM、ReGreetなど。

Trade-off: greetdは最小限のログイン管理を提供するため、GUI greeterへ移行する際にはCageなどの描画環境やセッション登録の設定が必要になる。

### ADR-008：外観と操作性のカスタマイズを後回しにする

採用

Decision: まず機能の成立を優先し、テーマ、配色、壁紙、キーバインドの細かな最適化は後から行う。

Rationale: 機能上の問題とカスタマイズによる問題を分離し、最小限の動作環境を早期に完成させるため。

Trade-off: 当面は各アプリケーションの標準UIが混在する。

## 5. 発生した問題と解決

### 音声デバイスがDummy Outputしか表示されない

対象ハードウェアはDell 14 Plus DB14250、Intel Core Ultra 200V（Lunar Lake）。

当初、PipeWireとWirePlumberは起動していたものの、`/proc/asound/cards`は`--- no soundcards ---`を表示していた。

カーネルドライバー`sof-audio-pci-intel-lnl`は認識されていたため、SOFファームウェアの不足を疑った。

`sof-firmware`と`alsa-utils`を導入して再起動したところ、ALSAに`sof-soundwire`が登録され、PipeWireにも内蔵スピーカー、ヘッドホン、HDMI出力、マイクが表示されるようになった。

Root cause: SOFファームウェアの未導入が原因だったと判断できる。

Resolution: `sof-firmware`の導入と再起動。なお、実際の音声再生テストの結果までは会話上で確認していない。

### PortalのRealtimeKit警告

`xdg-desktop-portal`は正常に起動していたが、RealtimeKitのD-Busサービスが見つからない警告が発生した。

Portal自体の致命的な障害ではなかったものの、PipeWireのリアルタイム処理にも有用なため、`rtkit`を追加導入する方針とした。

## 6. 現在の状態と残作業

第1〜第3段階はユーザーから完了報告済み。音声デバイス認識の復旧とWi-Fi接続も確認済みである。一方、追加段階のUWSM、greetd、キーボード配列変更については、実施手順を提示した段階であり、導入・動作確認の完了は確認できていない。

キー配列変更, greetd, uwsm導入済み

今後の作業は、UWSMとgreetdの実導入、セッションの自動起動・終了の検証、キーボード配列の修正、必要に応じたGUI greeterへの変更である。その後、常駐サービスのsystemd管理への移行、キーバインド整理、外観の統一を進める。

全体の設計方針は、機能ごとの責務を分離し、必要最小限の構成から段階的に拡張すること。 この方針を維持することで、今後の変更や問題発生時にも、各コンポーネントを比較的独立して管理できる。

## 導入パッケージ

hyprland
xorg-xwayland
kitty
hyprlauncher
waybar
mesa
noto-fonts
noto-fonts-cjk
noto-fonts-emoji
ttf-dejavu

xdg-desktop-portal
xdg-desktop-portal-hyprland
xdg-desktop-portal-gtk
hyprpolkitagent
pipewire
pipewire-pulse
pipewire-alsa
wireplumber
pavucontrol
networkmanager
network-manager-applet
hyprlock
hypridle
sof-firmware
alsa-utils
rtkit

swaync
hyprpaper
thunar
tumbler
gvfs
udisks2
ffmpegthumbnailer
wl-clipboard
cliphist
grim
slurp
brightnessctl
fuzzel
libnotify

uwsm
libnewt
greetd
greetd-tuigreet
