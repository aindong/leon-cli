#!/bin/bash
# traffic.sh [PORT]: a steady stream of requests, so the dev server's log fills in a few seconds.
port=${1:-8080}
paths=(/books /books/1 /books/2 /health "/books?shelf=software" /books/9 /books/3)
i=0
while :; do
  curl -s -o /dev/null "http://127.0.0.1:$port${paths[i++ % ${#paths[@]}]}"
  if (( i % 9 == 0 )); then
    curl -s -o /dev/null -X POST -d '{"title":"Notes","author":"Anon"}' "http://127.0.0.1:$port/books"
  fi
  sleep 0.$(( (RANDOM % 3) + 1 ))
done
