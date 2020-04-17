#!/bin/sh
date=`git show --pretty=%cI -s`
hash=`git show --pretty=%h -s`
echo ${date}.${hash}
