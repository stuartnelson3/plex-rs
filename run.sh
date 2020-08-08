#!/bin/bash

export RUST_LOG=plex_downloader=debug
eval `ssh-agent -s`
ssh-add

/var/lib/plexmediaserver/plex-downloader --server "admin@derp.biz" --split "prefix/" --port 4567
