complete -c fossil -f

complete -c fossil -n __fish_use_subcommand -a pack -d 'compress a file or directory'
complete -c fossil -n __fish_use_subcommand -a lift -d 'fossilize the clipboard'
complete -c fossil -n __fish_use_subcommand -a unpack -d 'restore the original'
complete -c fossil -n __fish_use_subcommand -a inspect -d 'per-block analysis'
complete -c fossil -n __fish_use_subcommand -a map -d 'entropy heatmap or block models'
complete -c fossil -n __fish_use_subcommand -a explain -d 'the reconstruction recipe'
complete -c fossil -n __fish_use_subcommand -a verify -d "check a fossil's CRC"
complete -c fossil -n __fish_use_subcommand -a update -d 'reinstall the latest fossil'
complete -c fossil -n __fish_use_subcommand -a help -d 'show help'

complete -c fossil -n '__fish_seen_subcommand_from pack' -l lossy -d 'quantize (drop low bits)'
complete -c fossil -n '__fish_seen_subcommand_from pack' -l best-effort -d 'already-compressed inputs lossless'
complete -c fossil -n '__fish_seen_subcommand_from pack' -l images-only -d 'lossy on raw images only'
complete -c fossil -n '__fish_seen_subcommand_from pack' -l verify -d 'round-trip check'
complete -c fossil -n '__fish_seen_subcommand_from pack lift' -l reveal -d 'reveal the .fossil after packing'
complete -c fossil -n '__fish_seen_subcommand_from lift' -l lossy -d 'quantize (drop low bits)'
complete -c fossil -n '__fish_seen_subcommand_from lift' -l best-effort -d 'already-compressed inputs lossless'
complete -c fossil -n '__fish_seen_subcommand_from lift' -l images-only -d 'lossy on raw images only'
complete -c fossil -n '__fish_seen_subcommand_from unpack' -l trust -d 'skip the CRC check'
complete -c fossil -n '__fish_seen_subcommand_from explain' -l block -d 'deep-dive a single block'
