export LESS="-FMRW"

export EDITOR=vim
export VISUAL=vim
export GIT_EDITOR=vim

alias lg='lazygit'
alias yz='yazi'

# Shell syntax like pips and '&&' cannot be used in `de`.
de() {
  case "$1" in
    up)
      shift
      devcontainer up --workspace-folder "$PWD" "$@"
      ;;

    stop)
      local ids
      ids=$(docker ps -q \
        --filter "label=devcontainer.local_folder=$PWD") || return 1
      if [[ -n "$ids" ]]; then
        docker stop "${(@f)ids}" || return 1
      else
        print "No running Dev Containers for this workspace."
      fi
      print "\nRunning containers:"
      docker ps --format "table {{.Names}}\t{{.Image}}\t{{.Status}}"
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
