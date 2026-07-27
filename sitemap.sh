#!/bin/bash

cd course
mdbook build
sscli -b https://skyzh.github.io/write-you-a-vector-db -r book -f xml -o > src/sitemap.xml
sscli -b https://skyzh.github.io/write-you-a-vector-db -r book -f txt -o > src/sitemap.txt
mdbook build
