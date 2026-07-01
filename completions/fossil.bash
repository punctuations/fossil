_fossil() {
    local cur
    cur="${COMP_WORDS[COMP_CWORD]}"

    local commands="pack lift unpack inspect map explain verify update help"

    if [ "$COMP_CWORD" -eq 1 ]; then
        COMPREPLY=( $(compgen -W "$commands --version --help" -- "$cur") )
        return 0
    fi

    case "${COMP_WORDS[1]}" in
        pack|bury|cover|ize)
            COMPREPLY=( $(compgen -W "--lossy --best-effort --images-only --verify --reveal" -- "$cur") )
            ;;
        lift|c-v|c/v)
            COMPREPLY=( $(compgen -W "--reveal --lossy --best-effort --images-only --verify" -- "$cur") )
            ;;
        unpack|recover|exhume|uncover)
            COMPREPLY=( $(compgen -W "--trust" -- "$cur") )
            ;;
        explain|why|describe)
            COMPREPLY=( $(compgen -W "--block" -- "$cur") )
            ;;
    esac

    COMPREPLY+=( $(compgen -f -- "$cur") )
    return 0
}
complete -F _fossil fossil
