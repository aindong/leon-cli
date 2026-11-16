#!/bin/bash
# frames.sh VIDEO OUTDIR T…: one 720 px frame per time, and 3x4 contact sheets of them
set -euo pipefail
v=$1 out=$2; shift 2
mkdir -p "$out"; rm -f "$out"/*.png
for t; do ffmpeg -loglevel error -y -ss "$t" -i "$v" -frames:v 1 -vf scale=720:-2 "$out/f$(printf %05.1f "$t").png"; done
th=$(ffprobe -v error -select_streams v:0 -show_entries stream=height -of csv=p=0 "$out/$(ls "$out" | head -1)") # tile height
ls "$out"/f*.png | split -l 12 - "$out/list."
n=1
for l in "$out"/list.*; do
  args=(); while read -r f; do args+=(-i "$f"); done <"$l"
  c=$(wc -l <"$l" | tr -d ' ')
  ffmpeg -loglevel error -y "${args[@]}" -filter_complex "$(for ((i = 0; i < c; i++)); do printf '[%d]' $i; done)xstack=inputs=$c:layout=$(python3 -c "print('|'.join(f'{(i%3)*720}_{(i//3)*$th}' for i in range($c)))")" "$out/sheet$n.png"
  rm "$l"; n=$((n + 1))
done
