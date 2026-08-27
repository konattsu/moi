export LESS="-FMRW"

export EDITOR=vim
export VISUAL=vim
export GIT_EDITOR=vim

alias lg='lazygit'
alias yz='yazi'

# Shell syntax like pips and '&&' cannot be used here.
de() {
  case "$1" in
    up)
      shift
      devcontainer up --workspace-folder "$PWD" "$@"
      ;;
    "")
      devcontainer exec --workspace-folder "$PWD" zsh
      ;;
    *)
      local cmd="${(j: :)${(q)@}}"
      devcontainer exec --workspace-folder "$PWD" zsh -ic "$cmd"
      ;;
  esac
}
