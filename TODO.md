# TODO

- Make the Neovim binary install choose the correct release archive for non-x86_64 Arch systems.
- Revisit Neovim clipboard behavior on native Linux; OSC52 may not be ideal when wl-clipboard/xclip are installed.
- Check first-run Neovim plugin installs from non-interactive or GUI-launched shells where nvm-provided npm may not be on PATH.
- 新しめのkeychain毎回`press enter to initialize key`みたいな警告出てありえん使いにくいので要検討. 3.0.xのやつっぽい. pacman経由だとこれ
- `:bd`でwindowも消えるので要確認
- toml, yaml のエラー見れる奴. 多分masonで
- Neo-treeの横幅はリサイズ後と閉じる直前にNeovim stateへ保存し、再表示時や次回起動時に同じ幅へ戻す。 再表示直後に既存windowへ入った場合も保存幅へ戻す。Neo-treeだけが残って全画面幅になった状態は保存しない。
- nvim系設定たくさんあるので見てみる
- devcontainer cli見たいな進捗表示
- autohotkey: 英語の時にcaps lock押すと死ぬ
- symlink表示. 子なら@^みたいな
