#!/bin/bash
set -euo pipefail
sh "$1" "$2" | awk '
    /^install_verified "\$download_dir\/candidate" "\$tag"$/ {
        print "install_release_pair \"$download_dir/candidate\" \"$tag\" \"$target\""
        found++
        next
    }
    { print }
    END { if (found != 1) exit 1 }
'
