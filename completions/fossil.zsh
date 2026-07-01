#compdef fossil

_fossil() {
    local -a commands
    commands=(
        'pack:compress a file or directory (no input packs the clipboard)'
        'lift:fossilize the clipboard, copy the .fossil back'
        'unpack:restore the original (verifies CRC)'
        'inspect:per-block analysis'
        'map:entropy heatmap or block models'
        'explain:the reconstruction recipe'
        'verify:check a fossil'\''s CRC without unpacking'
        'update:reinstall the latest fossil from git'
        'help:show help'
    )

    if (( CURRENT == 2 )); then
        _describe -t commands 'fossil command' commands
        return
    fi

    case "${words[2]}" in
        pack|bury|cover|ize)
            _arguments \
                '--lossy=[quantize, dropping low bits of each byte]' \
                '--best-effort[pack already-compressed inputs losslessly]' \
                '--images-only[only apply lossy to raw image formats]' \
                '--verify[verify the round-trip before writing]' \
                '*:file:_files'
            ;;
        unpack|recover|exhume|uncover)
            _arguments '--trust[skip the CRC check]' '*:file:_files'
            ;;
        explain|why|describe)
            _arguments '--block[deep-dive a single block]:block number:' '*:file:_files'
            ;;
        *)
            _files
            ;;
    esac
}

_fossil "$@"
