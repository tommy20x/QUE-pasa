{ pkgs ? import <nixpkgs> {
    overlays = [
      (import (fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"))
    ];
  }
}:
let vimLspDeps =
  if builtins.getEnv("NIX_VIM_LSP_ENABLED") == "yes"
    then ["rust-analyzer" "nodejs-18_x"]
    else [];
in
pkgs.mkShell {
  nativeBuildInputs = with pkgs; (
    builtins.concatLists[
      [
        rust-bin.stable.latest.default
        pkg-config
        openssl
      ]
      (lib.attrVals vimLspDeps pkgs)
    ]);

  shellHook = if builtins.getEnv("NIX_VIM_LSP_ENABLED") == "yes" then
    ''
      mkdir -p .vim

      coc_settings_target=.vim/coc-settings.json
      [[ ! -f $coc_settings_target || -L $coc_settings_target ]] || {
        echo "warn: cannot setup rust-analyzer path for vim coc, $coc_settings_target is not a symlink and may contain information that should not be overwritten" >&2
        return
      }

      coc="`mktemp`"
      cat <<EOF > "$coc"
      {
         "rust-analyzer.server.path": "`which rust-analyzer`"
      }
      EOF

      ln -sf "$coc" $coc_settings_target
    ''
    else "";
}
