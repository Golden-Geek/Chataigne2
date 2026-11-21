#!/usr/bin/env bash
set -euo pipefail

bundle=$1
architecture=$2
contents="$bundle/Contents"
frameworks="$contents/Frameworks"
name=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$contents/Info.plist")
binary="$contents/MacOS/$name"
[[ -x "$binary" ]]
libraries=("$frameworks/"*.dylib)
[[ ${#libraries[@]} -eq 5 ]]
for item in "$binary" "${libraries[@]}"; do
    [[ $(lipo -archs "$item") = "$architecture" ]]
    links=$(otool -L "$item" | awk 'NR > 1 {print $1}')
    while IFS= read -r link; do
        case "$link" in
            /System/Library/*|/usr/lib/*) ;;
            @rpath/lib*.dylib)
                [[ -f "$frameworks/${link#@rpath/}" ]] ;;
            *)
                echo "$item references unexpected library $link" >&2
                exit 1 ;;
        esac
    done <<< "$links"
    while IFS= read -r rpath; do
        [[ "$rpath" = "" || "$rpath" = '@executable_path/../Frameworks' ]]
    done < <(otool -l "$item" | awk '$1 == "cmd" && $2 == "LC_RPATH" { getline; getline; print $2 }')
done
[[ $(otool -L "$binary") = *'@rpath/libavcodec.'* ]]
[[ -f "$contents/Resources/licenses/video/FFmpeg-LGPL-3.0.txt" ]]
codesign --verify --deep --strict "$bundle"
printf '%s\n' "$binary"
